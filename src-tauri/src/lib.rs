use base64::Engine;
use image::RgbaImage;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub mod ocr;
pub mod translation;
use translation::translate_on_screen;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Application state storing the latest captured raw screenshot and active crop in memory.
pub struct AppState {
    pub current_screenshot: Mutex<Option<RgbaImage>>,
    pub latest_crop_image: Mutex<Option<RgbaImage>>,
    pub latest_crop: Mutex<Option<String>>,
}

/// Linux display server and desktop session detection helper.
#[derive(Debug, Clone)]
pub struct LinuxDisplayInfo {
    pub session_type: String,
    pub wayland_display: Option<String>,
    pub x11_display: Option<String>,
    pub gdk_backend: Option<String>,
    pub desktop: Option<String>,
}

impl LinuxDisplayInfo {
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                session_type: std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_string()),
                wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
                x11_display: std::env::var("DISPLAY").ok(),
                gdk_backend: std::env::var("GDK_BACKEND").ok(),
                desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                session_type: "native".to_string(),
                wayland_display: None,
                x11_display: None,
                gdk_backend: None,
                desktop: None,
            }
        }
    }

    pub fn is_wayland(&self) -> bool {
        self.session_type.eq_ignore_ascii_case("wayland") || self.wayland_display.is_some()
    }

    pub fn log_startup(&self) {
        #[cfg(target_os = "linux")]
        {
            println!("🖥️ [DeskLens] Display Server Environment Detected:");
            println!("   • XDG_SESSION_TYPE:    {}", self.session_type);
            println!("   • WAYLAND_DISPLAY:     {}", self.wayland_display.as_deref().unwrap_or("<unset>"));
            println!("   • DISPLAY (X11):       {}", self.x11_display.as_deref().unwrap_or("<unset>"));
            println!("   • GDK_BACKEND:         {}", self.gdk_backend.as_deref().unwrap_or("<auto>"));
            println!("   • XDG_CURRENT_DESKTOP: {}", self.desktop.as_deref().unwrap_or("<unknown>"));
        }
    }
}

/// Captures the primary display screen using xcap and returns the raw RgbaImage.
/// Gracefully handles multi-monitor enumeration and falls back to the first available monitor.
fn capture_screen_raw() -> Result<RgbaImage, String> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    if monitors.is_empty() {
        return Err("No active monitor found on system".to_string());
    }

    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .unwrap_or(&monitors[0]);

    monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture screen image: {e}"))
}

/// Helper to encode an RgbaImage into a base64 PNG data URL.
fn encode_png_data_url(image: &RgbaImage) -> Result<String, String> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {e}"))?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());
    Ok(format!("data:image/png;base64,{}", encoded))
}

/// Saves the latest cropped image to a temporary file and returns its path.
fn write_crop_to_temp_file(state: &tauri::State<'_, AppState>) -> Result<PathBuf, String> {
    let guard = state
        .latest_crop_image
        .lock()
        .map_err(|e| format!("Failed to lock crop image state: {e}"))?;

    let image = guard
        .as_ref()
        .ok_or_else(|| "No cropped image available".to_string())?;

    let temp_path = std::env::temp_dir().join("desklens_latest_crop.png");
    image
        .save_with_format(&temp_path, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to save temporary crop PNG: {e}"))?;

    Ok(temp_path)
}

/// Opens an external HTTP/HTTPS URL in the user's default system browser without truncation or blocking.
fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let escaped_url = url.replace('\'', "''");
        let ps_cmd = format!("Start-Process '{}'", escaped_url);
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Failed to launch default browser on Windows: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Use GdkX11 + Gio.AppInfo with the live X11 server timestamp so GNOME/Mutter
        // registers a valid startup sequence (_TIME<server_time>) and immediately raises
        // and focuses the browser window instead of opening the tab silently in the background.
        let py_script = r#"
import os, sys, time
try:
    import gi
    gi.require_version('Gtk', '3.0')
    gi.require_version('Gdk', '3.0')
    gi.require_version('GdkX11', '3.0')
    from gi.repository import Gtk, Gdk, GdkX11, Gio
    url = sys.argv[1]
    w = Gtk.Window()
    w.realize()
    server_time = GdkX11.x11_get_server_time(w.get_window())
    display = Gdk.Display.get_default()
    ctx = display.get_app_launch_context()
    ctx.set_timestamp(server_time)
    for k in ('GDK_BACKEND', 'WEBKIT_DISABLE_COMPOSITING_MODE', 'NO_AT_BRIDGE'):
        os.environ.pop(k, None)
        ctx.unsetenv(k)
    app = Gio.AppInfo.get_default_for_uri_scheme('https')
    if app:
        startup_id = ctx.get_startup_notify_id(app, [])
        display.sync()
        if startup_id:
            os.environ['DESKTOP_STARTUP_ID'] = startup_id
            os.environ['XDG_ACTIVATION_TOKEN'] = startup_id
        if app.launch_uris([url], None):
            display.sync()
            time.sleep(0.15)
            sys.exit(0)
except Exception:
    pass
sys.exit(1)
"#;

        let url_owned = url.to_string();
        std::thread::spawn(move || {
            let py_ok = Command::new("python3")
                .args(["-c", py_script, &url_owned])
                .env("GDK_BACKEND", "x11")
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !py_ok {
                let _ = Command::new("xdg-open")
                    .arg(&url_owned)
                    .env_remove("GDK_BACKEND")
                    .env_remove("WEBKIT_DISABLE_COMPOSITING_MODE")
                    .env_remove("NO_AT_BRIDGE")
                    .spawn();
            }
        });
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to launch default browser on macOS: {e}"))?;
    }

    Ok(())
}

/// Copies a PNG file from disk into the native OS clipboard.
fn copy_file_to_os_clipboard(temp_path: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $img = [System.Drawing.Image]::FromFile('{}'); [System.Windows.Forms.Clipboard]::SetImage($img); $img.Dispose()",
            temp_path.to_string_lossy().replace('\'', "''")
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.output()
            .map_err(|e| format!("Failed to copy image to Windows clipboard: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let path_str = temp_path.to_string_lossy().to_string();

        // 1. Try Wayland wl-copy
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Ok(file) = std::fs::File::open(temp_path) {
                if let Ok(mut child) = Command::new("wl-copy")
                    .args(["-t", "image/png"])
                    .stdin(file)
                    .spawn()
                {
                    if let Ok(status) = child.wait() {
                        if status.success() {
                            return Ok(());
                        }
                    }
                }
            }
        }

        // 2. Try X11 xclip
        if let Ok(child) = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "image/png", "-i", &path_str])
            .output()
        {
            if child.status.success() {
                return Ok(());
            }
        }

        // 3. Try arboard cross-platform clipboard
        if let Ok(img) = image::open(temp_path) {
            let rgba = img.to_rgba8();
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let img_data = arboard::ImageData {
                    width: rgba.width() as usize,
                    height: rgba.height() as usize,
                    bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
                };
                if cb.set_image(img_data).is_ok() {
                    return Ok(());
                }
            }
        }

        return Err("Failed to copy image to clipboard: install wl-clipboard (for Wayland) or xclip (for X11)".to_string());
    }

    #[allow(unreachable_code)]
    Ok(())
}

/// Closes the fullscreen overlay and hides the main window back to the background.
#[tauri::command]
async fn hide_overlay(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_always_on_top(false);
        let _ = window.set_fullscreen(false);
        let _ = window.hide();
    }
    Ok(())
}

/// Allows the frontend to manually trigger a fresh screenshot.
#[tauri::command]
async fn trigger_capture(app_handle: tauri::AppHandle) -> Result<String, String> {
    let raw_image = capture_screen_raw()?;
    let data_url = encode_png_data_url(&raw_image)?;

    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(mut lock) = state.current_screenshot.lock() {
            *lock = Some(raw_image);
        }
    }

    Ok(data_url)
}

/// Crops a user-selected rectangle from the current screenshot and stores it in memory.
#[tauri::command]
async fn crop_selection(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    screen_width: u32,
    screen_height: u32,
) -> Result<String, String> {
    let cropped = {
        let guard = state
            .current_screenshot
            .lock()
            .map_err(|e| format!("Failed to lock screenshot state: {e}"))?;

        let image = guard
            .as_ref()
            .ok_or_else(|| "No active screenshot stored for cropping".to_string())?;

        let img_w = image.width();
        let img_h = image.height();

        // Scale calculation to handle Windows/Linux Display Scaling (e.g. 125%, 150%, 200%)
        let scale_x = if screen_width > 0 {
            img_w as f64 / screen_width as f64
        } else {
            1.0
        };
        let scale_y = if screen_height > 0 {
            img_h as f64 / screen_height as f64
        } else {
            1.0
        };

        let real_x = ((x as f64) * scale_x).round() as u32;
        let real_y = ((y as f64) * scale_y).round() as u32;
        let real_w = ((width as f64) * scale_x).round() as u32;
        let real_h = ((height as f64) * scale_y).round() as u32;

        if real_x >= img_w || real_y >= img_h || real_w == 0 || real_h == 0 {
            return Err("Invalid selection dimensions".to_string());
        }

        let clamped_w = real_w.min(img_w - real_x);
        let clamped_h = real_h.min(img_h - real_y);

        println!("✂️ [DeskLens] Successfully cropped region: {clamped_w}x{clamped_h} px");
        image::imageops::crop_imm(image, real_x, real_y, clamped_w, clamped_h).to_image()
    };

    let data_url = encode_png_data_url(&cropped)?;

    // Cache both raw cropped image and data URL in state
    if let Ok(mut img_lock) = state.latest_crop_image.lock() {
        *img_lock = Some(cropped);
    }
    if let Ok(mut crop_lock) = state.latest_crop.lock() {
        *crop_lock = Some(data_url.clone());
    }

    // Broadcast globally so all windows receive the new payload
    let _ = app_handle.emit("load-search-payload", &data_url);

    Ok(data_url)
}

/// Retrieves the latest cropped image data URL from state.
#[tauri::command]
async fn get_latest_crop(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let guard = state
        .latest_crop
        .lock()
        .map_err(|e| format!("Failed to lock crop state: {e}"))?;
    Ok(guard.clone())
}

/// Helper to ensure the "results" side-panel window exists and is shown.
/// Must be called from an async command so WebView2 creation never blocks the main UI thread.
fn ensure_results_window(app_handle: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(win) = app_handle.get_webview_window("results") {
        let _ = win.set_size(tauri::LogicalSize::new(430.0, 720.0));
        return Ok(win);
    }

    WebviewWindowBuilder::new(
        app_handle,
        "results",
        WebviewUrl::App("results.html".into()),
    )
    .title("DeskLens — Visual Search")
    .inner_size(430.0, 720.0)
    .min_inner_size(380.0, 520.0)
    .always_on_top(true)
    .decorations(true)
    .visible(true)
    .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
    .build()
    .map_err(|e| format!("Failed to create results window: {e}"))
}

/// SPRINT 3: Opens the integrated side-panel Visual Search window with the cropped image.
#[tauri::command]
async fn open_visual_search(
    app_handle: tauri::AppHandle,
    crop_data_url: Option<String>,
) -> Result<(), String> {
    // 1. Hide the fullscreen capture overlay
    if let Some(main_win) = app_handle.get_webview_window("main") {
        let _ = main_win.set_fullscreen(false);
        let _ = main_win.hide();
    }

    // 2. Cache crop payload in state if provided
    let mut payload_to_send: Option<String> = None;
    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(mut crop_lock) = state.latest_crop.lock() {
            if let Some(url) = crop_data_url {
                *crop_lock = Some(url.clone());
                payload_to_send = Some(url);
            } else {
                payload_to_send = crop_lock.clone();
            }
        }
    }

    // 3. Ensure Results side-panel exists, unminimize if minimized, bring to front, and broadcast payload
    let results_win = ensure_results_window(&app_handle)?;
    let _ = results_win.unminimize();
    let _ = results_win.show();
    let _ = results_win.set_always_on_top(false);
    let _ = results_win.set_always_on_top(true);
    let _ = results_win.set_focus();

    if let Some(payload) = payload_to_send {
        let _ = app_handle.emit("load-search-payload", &payload);
    }

    println!("🔍 [DeskLens] Visual Search side-panel opened and focused.");
    Ok(())
}

/// Simple URL-encoder for query parameter values (RFC 3986).
fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Uploads the cropped image to uguu.se (~500ms, serves direct image/png).
fn upload_to_uguu(temp_path: &PathBuf) -> Result<String, String> {
    let temp_path_str = temp_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    let curl_bin = "curl.exe";
    #[cfg(not(target_os = "windows"))]
    let curl_bin = "curl";

    let file_arg = format!("files[]=@{}", temp_path_str);
    let mut cmd = Command::new(curl_bin);
    cmd.args([
        "-s",
        "--max-time",
        "5",
        "-F",
        &file_arg,
        "https://uguu.se/upload",
    ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run uguu upload: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(files) = json["files"].as_array() {
            if let Some(first) = files.first() {
                if let Some(url) = first["url"].as_str() {
                    return Ok(url.to_string());
                }
            }
        }
    }

    Err(format!(
        "uguu upload failed: {}",
        stdout.chars().take(80).collect::<String>()
    ))
}

/// Uploads the cropped image to kappa.lol (~600ms fallback, serves direct image/png).
fn upload_to_kappa(temp_path: &PathBuf) -> Result<String, String> {
    let temp_path_str = temp_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    let curl_bin = "curl.exe";
    #[cfg(not(target_os = "windows"))]
    let curl_bin = "curl";

    let file_arg = format!("file=@{}", temp_path_str);
    let mut cmd = Command::new(curl_bin);
    cmd.args([
        "-s",
        "--max-time",
        "5",
        "-F",
        &file_arg,
        "https://kappa.lol/api/upload",
    ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run kappa.lol upload: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(link) = json["link"].as_str() {
            let ext = json["ext"].as_str().unwrap_or(".png");
            if link.ends_with(ext) {
                return Ok(link.to_string());
            } else {
                return Ok(format!("{}{}", link, ext));
            }
        }
    }

    Err(format!(
        "kappa.lol upload failed: {}",
        stdout.chars().take(80).collect::<String>()
    ))
}

/// Uploads the cropped image to freeimage.host as tertiary fallback.
fn upload_to_freeimage(temp_path: &PathBuf) -> Result<String, String> {
    let temp_path_str = temp_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    let curl_bin = "curl.exe";
    #[cfg(not(target_os = "windows"))]
    let curl_bin = "curl";

    let file_arg = format!("source=@{}", temp_path_str);
    let mut cmd = Command::new(curl_bin);
    cmd.args([
        "-s",
        "--max-time",
        "6",
        "-F",
        "key=6d207e02198a847aa98d0a2a901485a5",
        "-F",
        "action=upload",
        "-F",
        &file_arg,
        "-F",
        "format=json",
        "https://freeimage.host/api/1/upload",
    ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run upload command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(url) = json["image"]["url"].as_str() {
            return Ok(url.to_string());
        }
        if let Some(url) = json["image"]["display_url"].as_str() {
            return Ok(url.to_string());
        }
    }

    Err(format!(
        "Upload failed: {}",
        stdout.chars().take(80).collect::<String>()
    ))
}

/// Immediately hides the overlay, uploads the cropped image to a raw-image CDN (`image/png`),
/// and opens `https://lens.google.com/uploadbyurl?url=...` in the foreground browser window.
#[tauri::command]
async fn search_google_lens(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // 1. Immediately hide the fullscreen overlay so the user never has to press ESC
    //    and the always-on-top window never blocks the browser from taking focus.
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_always_on_top(false);
        let _ = window.set_fullscreen(false);
        let _ = window.hide();
    }
    let _ = app_handle.emit("view-mode", "settings");

    let temp_path = write_crop_to_temp_file(&state)?;
    let _ = copy_file_to_os_clipboard(&temp_path);

    println!("🌐 [DeskLens] Uploading cropped slice to raw-image visual search CDN...");

    let upload_path = temp_path.clone();
    let host_result = tauri::async_runtime::spawn_blocking(move || {
        if let Ok(url) = upload_to_uguu(&upload_path) {
            println!("⚡ [DeskLens] Fast upload via uguu.se succeeded: {url}");
            return Ok(url);
        }
        if let Ok(url) = upload_to_kappa(&upload_path) {
            println!("⚡ [DeskLens] Fast upload via kappa.lol succeeded: {url}");
            return Ok(url);
        }
        println!("⚠️ [DeskLens] Trying freeimage.host fallback...");
        upload_to_freeimage(&upload_path)
    })
    .await
    .map_err(|e| format!("Image upload task failed: {e}"))?;

    match host_result {
        Ok(direct_img_url) => {
            let encoded = url_encode(&direct_img_url);
            let lens_url = format!("https://lens.google.com/uploadbyurl?url={encoded}&hl=pt-BR");
            println!("🔍 [DeskLens] Launching Google Lens by URL: {lens_url}");
            open_external_url(&lens_url)?;
            Ok(lens_url)
        }
        Err(err) => {
            eprintln!("⚠️ [DeskLens] Hosting failed ({err}). Opening Google Images (image is on the clipboard)...");
            let fallback_url = "https://images.google.com/";
            open_external_url(fallback_url)?;
            Ok(fallback_url.to_string())
        }
    }
}

/// Copies the cropped image to the system clipboard and opens Google Images in the default browser.
#[tauri::command]
async fn open_search_provider(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> Result<(), String> {
    if let Ok(temp_path) = write_crop_to_temp_file(&state) {
        let _ = copy_file_to_os_clipboard(&temp_path);
    }

    let url = match provider.as_str() {
        "google-images" => "https://images.google.com/",
        _ => "https://lens.google.com/",
    };

    open_external_url(url)
}

/// Copies the cropped image directly to the native OS Clipboard.
#[tauri::command]
async fn copy_crop_to_clipboard(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let temp_path = write_crop_to_temp_file(&state)?;
    copy_file_to_os_clipboard(&temp_path)?;
    println!("📋 [DeskLens] Cropped image copied to system clipboard.");
    Ok(())
}

/// Saves the cropped PNG image to the user's Pictures/DeskLens directory and highlights it in Explorer.
#[tauri::command]
async fn save_crop_to_disk(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let file_path = {
        let guard = state
            .latest_crop_image
            .lock()
            .map_err(|e| format!("Failed to lock crop image state: {e}"))?;

        let image = guard
            .as_ref()
            .ok_or_else(|| "No cropped image available to save".to_string())?;

        let base_dir = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());

        let save_dir = base_dir.join("Pictures").join("DeskLens");
        std::fs::create_dir_all(&save_dir)
            .map_err(|e| format!("Failed to create Pictures/DeskLens folder: {e}"))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let path = save_dir.join(format!("desklens-crop-{timestamp}.png"));
        image
            .save_with_format(&path, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to write PNG file: {e}"))?;
        path
    };

    let path_str = file_path.to_string_lossy().to_string();
    println!("💾 [DeskLens] Saved cropped PNG to: {path_str}");

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer.exe")
            .arg(format!("/select,{}", path_str))
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(dir) = file_path.parent() {
            let _ = Command::new("xdg-open")
                .arg(dir)
                .spawn();
        }
    }

    Ok(path_str)
}

/// Copies plain text to the native operating system clipboard.
pub fn copy_plain_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows::ApplicationModel::DataTransfer::{Clipboard, DataPackage};
        let package =
            DataPackage::new().map_err(|e| format!("Failed to create DataPackage: {e}"))?;
        package
            .SetText(&windows::core::HSTRING::from(text))
            .map_err(|e| format!("Failed to set clipboard text: {e}"))?;
        Clipboard::SetContent(&package)
            .map_err(|e| format!("Failed to set clipboard content: {e}"))?;
        let _ = Clipboard::Flush();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Try arboard cross-platform clipboard
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.set_text(text.to_string()).is_ok() {
                return Ok(());
            }
        }

        // 2. Fallback: wl-copy (Wayland)
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if let Ok(mut child) = Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(text.as_bytes());
                }
                if let Ok(status) = child.wait() {
                    if status.success() {
                        return Ok(());
                    }
                }
            }
        }

        // 3. Fallback: xclip (X11)
        if let Ok(mut child) = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return Ok(());
                }
            }
        }

        // 4. Fallback: xsel (X11)
        if let Ok(mut child) = Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    return Ok(());
                }
            }
        }

        return Err("Failed to copy text to clipboard: install wl-clipboard (for Wayland) or xclip (for X11)".to_string());
    }

    #[allow(unreachable_code)]
    Ok(())
}

/// SPRINT 4: Extracts text and bounding boxes from the latest cropped image using native OS OCR.
#[tauri::command]
async fn extract_text_from_crop(
    state: tauri::State<'_, AppState>,
) -> Result<ocr::OcrResult, String> {
    let guard = state
        .latest_crop_image
        .lock()
        .map_err(|e| format!("Failed to lock crop image state: {e}"))?;

    let image = guard
        .as_ref()
        .ok_or_else(|| "No cropped image available for OCR".to_string())?;

    let result = ocr::extract_text_native(image)?;
    println!(
        "📝 [DeskLens] OCR extracted {} lines ({} words)",
        result.lines.len(),
        result.word_count
    );
    Ok(result)
}

/// SPRINT 4: Extracts text from the crop and immediately copies it to the system clipboard.
#[tauri::command]
async fn copy_text_from_crop(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let guard = state
        .latest_crop_image
        .lock()
        .map_err(|e| format!("Failed to lock crop image state: {e}"))?;

    let image = guard
        .as_ref()
        .ok_or_else(|| "No cropped image available for OCR".to_string())?;

    let result = ocr::extract_text_native(image)?;
    let text = result.full_text.trim().to_string();

    if !text.is_empty() {
        copy_plain_text_to_clipboard(&text)?;
        println!(
            "📋 [DeskLens] OCR text copied to clipboard: '{}'",
            text.chars().take(40).collect::<String>()
        );
    }

    Ok(text)
}

/// Copies arbitrary plain text directly to the native OS clipboard.
#[tauri::command]
async fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    copy_plain_text_to_clipboard(&text)
}

/// Linux IPC listener: listens on /tmp/desklens_<uid>.sock for global hotkey trigger signals.
#[cfg(target_os = "linux")]
fn start_ipc_listener(app_handle: tauri::AppHandle) {
    let uid = unsafe { libc::getuid() };
    let socket_path = format!("/tmp/desklens_{}.sock", uid);
    let _ = std::fs::remove_file(&socket_path);

    std::thread::spawn(move || {
        use std::io::Read;
        use std::os::unix::net::UnixListener;

        let listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("⚠️ [DeskLens IPC] Could not bind socket {}: {}", socket_path, e);
                return;
            }
        };

        println!("🚀 [DeskLens IPC] Listening on {} for global trigger signals.", socket_path);

        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buffer = [0u8; 128];
                if let Ok(n) = stream.read(&mut buffer) {
                    let msg = String::from_utf8_lossy(&buffer[..n]);
                    if msg.contains("trigger") {
                        println!("🎯 [DeskLens IPC] Received trigger command from system shortcut!");
                        capture_and_display_overlay(&app_handle);
                    }
                }
            }
        }
    });
}

/// Triggers screen capture and presents the fullscreen interactive overlay.
fn capture_and_display_overlay(app_handle: &tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        // Hide results panel before capturing so it doesn't obstruct the screen
        if let Some(results_win) = app_handle.get_webview_window("results") {
            let _ = results_win.hide();
        }

        // 1. Capture clean screen before opening overlay
        match capture_screen_raw() {
            Ok(raw_image) => {
                if let Ok(data_url) = encode_png_data_url(&raw_image) {
                    // Store image in AppState for instant cropping
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(mut lock) = state.current_screenshot.lock() {
                            *lock = Some(raw_image);
                        }
                    }

                    // 2. Broadcast captured image globally to frontend
                    let _ = app_handle.emit("screenshot-captured", &data_url);
                    let _ = app_handle.emit("view-mode", "overlay");

                    // 3. Show window first so GTK realizes it properly before entering fullscreen
                    let _ = window.show();
                    let _ = window.set_fullscreen(true);
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_focus();
                    println!("✨ [DeskLens] Screen captured and overlay active.");
                }
            }
            Err(err) => {
                eprintln!("❌ [DeskLens] Error capturing screen: {err}");
            }
        }
    }
}

/// Closes the visual search results window.
#[tauri::command]
async fn close_results_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(results_win) = app_handle.get_webview_window("results") {
        let _ = results_win.hide();
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let display_info = LinuxDisplayInfo::detect();
    display_info.log_startup();

    tauri::Builder::default()
        .manage(AppState {
            current_screenshot: Mutex::new(None),
            latest_crop_image: Mutex::new(None),
            latest_crop: Mutex::new(None),
        })
        .on_window_event(|window, event| {
            // Prevent "main" and "results" windows from being destroyed when clicking the OS 'X' button
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == "main" || label == "results" {
                    api.prevent_close();
                    let _ = window.set_fullscreen(false);
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            hide_overlay,
            trigger_capture,
            crop_selection,
            get_latest_crop,
            open_visual_search,
            search_google_lens,
            open_search_provider,
            copy_crop_to_clipboard,
            save_crop_to_disk,
            close_results_window,
            extract_text_from_crop,
            copy_text_from_crop,
            copy_text_to_clipboard,
            translate_on_screen
        ])
        .setup(move |app| {
            #[cfg(target_os = "linux")]
            start_ipc_listener(app.handle().clone());

            // On macOS, hide dock icon (Accessory mode)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // =========================================================================
            // SYSTEM TRAY: Interactive Menu (Capture, Settings & Quit)
            // =========================================================================
            let capture_item = MenuItem::with_id(
                app,
                "capture",
                "Capture Screen (Ctrl+Shift+X)",
                true,
                None::<&str>,
            )?;
            let settings_item = MenuItem::with_id(
                app,
                "settings",
                "Settings",
                true,
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(
                app,
                "quit",
                "Quit",
                true,
                None::<&str>,
            )?;

            let tray_menu = Menu::with_items(app, &[&capture_item, &settings_item, &quit_item])?;

            let tray_builder = TrayIconBuilder::new()
                .menu(&tray_menu)
                .tooltip("DeskLens — Google Lens for Desktop")
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "capture" => {
                        println!("📸 [DeskLens] Screen capture triggered from System Tray...");
                        capture_and_display_overlay(app);
                    }
                    "settings" => {
                        println!("⚙️ [DeskLens] Opening Settings window...");
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_fullscreen(false);
                            let _ = window.set_size(tauri::LogicalSize::new(520.0, 380.0));
                            let _ = window.center();
                            let _ = app.emit("view-mode", "settings");
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        println!("👋 [DeskLens] Exiting DeskLens from System Tray...");
                        app.exit(0);
                    }
                    _ => {}
                });

            let tray_icon = app.default_window_icon().cloned().unwrap_or_else(|| {
                let raw_bytes = include_bytes!("../icons/128x128.png");
                let img = image::load_from_memory(raw_bytes).expect("Valid 128x128 tray icon asset");
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                tauri::image::Image::new_owned(rgba.into_raw(), w, h)
            });

            let _tray = tray_builder.icon(tray_icon).build(app)?;

            // =========================================================================
            // GLOBAL HOTKEY: Ctrl + Shift + X (Capture & Overlay)
            // =========================================================================
            let ctrl_shift_x = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::KeyX,
            );

            let app_handle = app.handle().clone();

            // Register global shortcut plugin safely without ? to avoid crashing on init failure
            if let Err(plugin_err) = app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |_app, shortcut, event| {
                        if shortcut == &ctrl_shift_x && event.state() == ShortcutState::Pressed {
                            println!(
                                "🎯 [DeskLens] Global shortcut (Ctrl + Shift + X) triggered! Capturing screen..."
                            );
                            capture_and_display_overlay(&app_handle);
                        }
                    })
                    .build(),
            ) {
                eprintln!("⚠️ [DeskLens] Failed to initialize global shortcut plugin: {plugin_err}");
            }

            match app.global_shortcut().register(ctrl_shift_x) {
                Ok(()) => {
                    println!("🚀 [DeskLens] Global shortcut (Ctrl + Shift + X) registered successfully.");
                    if display_info.is_wayland() && display_info.gdk_backend.as_deref() != Some("x11") {
                        println!("ℹ️ [DeskLens] Note: Running on Wayland with native backend.");
                        println!("   Wayland restricts global hotkey capture when non-Xwayland windows are focused.");
                        println!("   • Tip: Run with 'GDK_BACKEND=x11 ./run-linux.sh' for system-wide shortcuts,");
                        println!("   • Or use the System Tray icon menu ('Capture Screen').");
                    }
                }
                Err(err) => {
                    eprintln!("⚠️ [DeskLens] Global shortcut (Ctrl+Shift+X) registration warning: {err}");
                    if display_info.is_wayland() {
                        eprintln!("ℹ️ [DeskLens] Wayland Security Policy: Direct global key grabbing is restricted by the compositor.");
                        eprintln!("   • Solution 1: Run with 'GDK_BACKEND=x11 ./run-linux.sh' to enable Xwayland shortcut interception.");
                        eprintln!("   • Solution 2: Trigger screen capture anytime from the System Tray icon menu.");
                        eprintln!("   • Solution 3: Bind a custom shortcut in your desktop settings (e.g. GNOME Settings -> Keyboard -> Shortcuts).");
                    } else {
                        eprintln!("ℹ️ [DeskLens] The shortcut may be reserved or conflicting. Screen capture is still available via System Tray.");
                    }
                }
            }

            println!("🚀 [DeskLens] Invisible engine started in system tray.");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running DeskLens Tauri application");
}
