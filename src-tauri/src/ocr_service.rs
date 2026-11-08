//! Immutable crop sessions and the persistent, offline OCR process.
use image::RgbaImage;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    sync::{mpsc, Arc, Mutex, OnceLock},
    time::Duration,
};

type SharedResult = Arc<OnceLock<Result<Value, String>>>;
#[derive(Clone, Serialize)]
pub struct CropInfo {
    pub crop_id: String,
    pub data_url: String,
    pub image_width: u32,
    pub image_height: u32,
}
struct Session {
    info: CropInfo,
    image: Arc<RgbaImage>,
    results: HashMap<String, SharedResult>,
}
pub struct OcrService {
    session: Mutex<Option<Session>>,
    next: AtomicU64,
    worker: Mutex<Worker>,
}

impl Default for OcrService {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
            next: AtomicU64::new(0),
            worker: Mutex::new(Worker::default()),
        }
    }
}
impl OcrService {
    pub fn set_crop(&self, image: RgbaImage, data_url: String) -> CropInfo {
        let id = self.reserve_crop();
        self.finish_crop(&id, image, data_url)
            .expect("sequential session creation")
    }
    pub fn reserve_crop(&self) -> String {
        let mut state = self.session.lock().unwrap();
        *state = None;
        (self.next.fetch_add(1, Ordering::Relaxed) + 1).to_string()
    }
    pub fn finish_crop(
        &self,
        id: &str,
        image: RgbaImage,
        data_url: String,
    ) -> Result<CropInfo, String> {
        let mut state = self.session.lock().map_err(|e| e.to_string())?;
        if id != self.next.load(Ordering::Relaxed).to_string() {
            return Err("This selection is no longer active".into());
        }
        let info = CropInfo {
            crop_id: id.to_string(),
            image_width: image.width(),
            image_height: image.height(),
            data_url,
        };
        *state = Some(Session {
            info: info.clone(),
            image: Arc::new(image),
            results: HashMap::new(),
        });
        Ok(info)
    }
    pub fn clear(&self) {
        let _ = self.reserve_crop();
    }
    pub fn clear_if_current(&self, id: &str) {
        let mut state = self.session.lock().unwrap();
        if state.as_ref().is_some_and(|s| s.info.crop_id == id) {
            *state = None;
            self.next.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn current_info(&self) -> Option<CropInfo> {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.info.clone())
    }
    pub fn current_image(&self) -> Result<Arc<RgbaImage>, String> {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.image.clone())
            .ok_or_else(|| "No cropped image available".into())
    }
    pub fn is_current(&self, id: &str) -> bool {
        self.session
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|s| s.info.crop_id == id)
    }
    pub fn recognize(&self, id: &str, mode: &str, resource_dir: PathBuf) -> Result<Value, String> {
        self.recognize_with(id, mode, |image| {
            if !cfg!(feature = "ocr-v2") {
                return native_document(image);
            }
            let temp = tempfile::Builder::new()
                .prefix("desklens-ocr-")
                .suffix(".png")
                .tempfile()
                .map_err(|e| e.to_string())?;
            image
                .save_with_format(temp.path(), image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            self.worker.lock().map_err(|e| e.to_string())?.recognize(
                &resource_dir,
                temp.path(),
                id,
                mode,
            )
        })
    }
    fn recognize_with<F>(&self, id: &str, mode: &str, infer: F) -> Result<Value, String>
    where
        F: FnOnce(&RgbaImage) -> Result<Value, String>,
    {
        if !["auto", "text", "code"].contains(&mode) {
            return Err("Invalid OCR mode".into());
        }
        let (image, info, cell) = {
            let mut state = self.session.lock().map_err(|e| e.to_string())?;
            let s = state
                .as_mut()
                .filter(|s| s.info.crop_id == id)
                .ok_or("This selection is no longer active")?;
            let cell = s
                .results
                .entry(mode.to_string())
                .or_insert_with(|| Arc::new(OnceLock::new()))
                .clone();
            (s.image.clone(), s.info.clone(), cell)
        };
        let result = cell
            .get_or_init(|| {
                let mut result = infer(&image)?;
                result["crop_id"] = json!(id);
                result["mode"] = json!(mode);
                result["image_width"] = json!(info.image_width);
                result["image_height"] = json!(info.image_height);
                if !result["full_text"].is_string() || !result["lines"].is_array() {
                    return Err("Invalid OCR document".into());
                }
                Ok(result)
            })
            .clone();
        let mut state = self.session.lock().map_err(|e| e.to_string())?;
        let session = state
            .as_mut()
            .filter(|s| s.info.crop_id == id)
            .ok_or("This selection is no longer active")?;
        if result.is_err()
            && session
                .results
                .get(mode)
                .is_some_and(|existing| Arc::ptr_eq(existing, &cell))
        {
            session.results.remove(mode);
        }
        result
    }
    pub fn retry(&self, id: &str, mode: &str) -> Result<(), String> {
        let mut state = self.session.lock().map_err(|e| e.to_string())?;
        let session = state
            .as_mut()
            .filter(|s| s.info.crop_id == id)
            .ok_or("This selection is no longer active")?;
        if let Some(cell) = session.results.get(mode) {
            if cell.get().is_none() {
                return Err("OCR is still processing this selection".into());
            }
        }
        session.results.remove(mode);
        Ok(())
    }
}

// Preserve the default engine until cross-platform promotion gates pass.
fn native_document(image: &RgbaImage) -> Result<Value, String> {
    let native = crate::ocr::extract_text_native(image)?;
    let mut result = serde_json::to_value(native).map_err(|e| e.to_string())?;
    let full = result["full_text"].as_str().unwrap_or("").to_owned();
    let mut cursor = 0;
    if let Some(lines) = result["lines"].as_array_mut() {
        for line in lines {
            let text = line["text"].as_str().unwrap_or("").to_owned();
            if let Some(relative) = full[cursor..].find(&text) {
                let start = cursor + relative;
                line["start_utf16"] = json!(full[..start].encode_utf16().count());
                line["end_utf16"] = json!(full[..start + text.len()].encode_utf16().count());
                let mut word_cursor = start;
                if let Some(words) = line["words"].as_array_mut() {
                    for word in words {
                        let token = word["text"].as_str().unwrap_or("").to_owned();
                        if let Some(relative) = full[word_cursor..start + text.len()].find(&token) {
                            let begin = word_cursor + relative;
                            word["start_utf16"] = json!(full[..begin].encode_utf16().count());
                            word["end_utf16"] =
                                json!(full[..begin + token.len()].encode_utf16().count());
                            word_cursor = begin + token.len();
                        }
                    }
                }
                cursor = start + text.len();
            }
        }
    }
    result["engine"] = json!("native");
    Ok(result)
}

struct Process {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Result<Value, String>>,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[derive(Default)]
struct Worker {
    process: Option<Process>,
    next: u64,
}
impl Worker {
    fn start(&mut self, resource_dir: &std::path::Path) -> Result<(), String> {
        let executable = if cfg!(windows) {
            "desklens-ocr.exe"
        } else {
            "desklens-ocr"
        };
        let packaged = resource_dir.join("ocr-runtime").join(executable);
        let mut command = if packaged.is_file() {
            Command::new(packaged)
        } else {
            #[cfg(debug_assertions)]
            {
                let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .to_path_buf();
                let python = root.join(if cfg!(windows) {
                    ".venv-ocr/Scripts/python.exe"
                } else {
                    ".venv-ocr/bin/python"
                });
                let mut command = Command::new(python);
                command
                    .arg(root.join("tools/ocr_worker/worker.py"))
                    .arg("--model-dir")
                    .arg(root.join(".ocr-models"));
                command
            }
            #[cfg(not(debug_assertions))]
            {
                return Err("The bundled OCR engine is missing. Reinstall DeskLens.".into());
            }
        };
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Could not start local OCR engine: {e}"))?;
        let input = child.stdin.take().ok_or("OCR input pipe unavailable")?;
        let stdout = child.stdout.take().ok_or("OCR output pipe unavailable")?;
        let (tx, output) = mpsc::sync_channel(2);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let parsed = line.map_err(|e| e.to_string()).and_then(|line| {
                    serde_json::from_str(&line).map_err(|e| format!("Invalid OCR protocol: {e}"))
                });
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        let process = Process {
            child,
            input,
            output,
        };
        let hello = process
            .output
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "OCR initialization timed out or engine exited")??;
        if hello["type"] != "ready" || hello["protocol_version"] != 1 {
            return Err("Incompatible OCR engine protocol".into());
        }
        self.process = Some(process);
        Ok(())
    }
    fn recognize(
        &mut self,
        resource_dir: &std::path::Path,
        path: &std::path::Path,
        crop_id: &str,
        mode: &str,
    ) -> Result<Value, String> {
        let mut failure = String::new();
        for _ in 0..2 {
            if self.process.is_none() {
                if let Err(e) = self.start(resource_dir) {
                    failure = e;
                    continue;
                }
            }
            self.next += 1;
            let id = self.next.to_string();
            let request = json!({"id": id, "crop_id": crop_id, "mode": mode, "input_path": path});
            let attempt = (|| {
                let process = self.process.as_mut().unwrap();
                writeln!(process.input, "{}", request)
                    .and_then(|_| process.input.flush())
                    .map_err(|e| e.to_string())?;
                let response = process
                    .output
                    .recv_timeout(Duration::from_secs(15))
                    .map_err(|_| "OCR timed out or engine exited".to_string())??;
                if response["id"] != id || response["crop_id"] != crop_id {
                    return Err("OCR response does not match selection".into());
                }
                if response["ok"] != true {
                    return Err(response["error"]
                        .as_str()
                        .unwrap_or("OCR recognition failed")
                        .to_string());
                }
                Ok(response["result"].clone())
            })();
            match attempt {
                Ok(result) => return Ok(result),
                Err(e) => {
                    failure = e;
                    self.process = None;
                }
            }
        }
        Err(failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_session_updates_image_and_payload_together() {
        let service = OcrService::default();
        let first = service.set_crop(RgbaImage::new(10, 20), "first".into());
        let second = service.set_crop(RgbaImage::new(30, 40), "second".into());
        assert!(!service.is_current(&first.crop_id));
        assert!(service.is_current(&second.crop_id));
        assert_eq!(service.current_image().unwrap().width(), 30);
        assert_eq!(service.current_info().unwrap().data_url, "second");
        service.clear();
        assert!(service.current_info().is_none());
    }
    #[test]
    fn stale_recognition_never_launches_worker() {
        let service = OcrService::default();
        assert!(service
            .recognize("missing", "auto", PathBuf::new())
            .unwrap_err()
            .contains("no longer active"));
    }
    #[test]
    fn slow_crop_cannot_replace_newer_selection() {
        let service = OcrService::default();
        let first = service.reserve_crop();
        let second = service.reserve_crop();
        let info = service
            .finish_crop(&second, RgbaImage::new(5, 5), "second".into())
            .unwrap();
        assert!(service
            .finish_crop(&first, RgbaImage::new(2, 2), "first".into())
            .is_err());
        service.clear_if_current(&first);
        assert!(service.is_current(&info.crop_id));
    }
    #[test]
    fn concurrent_requests_share_inference_and_modes_have_separate_cache() {
        let service = OcrService::default();
        let info = service.set_crop(RgbaImage::new(2, 2), "image".into());
        let calls = AtomicU64::new(0);
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..8 {
                handles.push(scope.spawn(|| {
                    service
                        .recognize_with(&info.crop_id, "auto", |_| {
                            calls.fetch_add(1, Ordering::Relaxed);
                            std::thread::sleep(Duration::from_millis(20));
                            Ok(json!({"full_text":"a😀", "lines":[]}))
                        })
                        .unwrap()
                }));
            }
            for handle in handles {
                assert_eq!(handle.join().unwrap()["full_text"], "a😀");
            }
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        service
            .recognize_with(&info.crop_id, "code", |_| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(json!({"full_text":"code", "lines":[]}))
            })
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
    #[test]
    fn failures_can_retry_and_inflight_stale_results_are_rejected() {
        let service = OcrService::default();
        let info = service.set_crop(RgbaImage::new(2, 2), "image".into());
        assert!(service
            .recognize_with(&info.crop_id, "auto", |_| Err("failure".into()))
            .is_err());
        assert!(service
            .recognize_with(&info.crop_id, "auto", |_| Ok(
                json!({"full_text":"retry", "lines":[]})
            ))
            .is_ok());
        let stale = service.recognize_with(&info.crop_id, "text", |_| {
            service.clear();
            Ok(json!({"full_text":"stale", "lines":[]}))
        });
        assert!(stale.unwrap_err().contains("no longer active"));
    }
}
