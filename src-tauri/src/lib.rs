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

/// Captures the primary display screen using xcap and returns the raw RgbaImage.
fn capture_screen_raw() -> Result<RgbaImage, String> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| xcap::Monitor::all().ok()?.into_iter().next())
        .ok_or_else(|| "No active monitor found".to_string())?;

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

/// Opens an external HTTP/HTTPS URL in the user's default system browser without blocking.
fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("rundll32.exe");
        cmd.args(["url.dll,FileProtocolHandler", url]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Failed to launch default browser on Windows: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to launch default browser on Linux: {e}"))?;
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
    }

    Ok(())
}

/// Closes the fullscreen overlay and hides the main window back to the background.
#[tauri::command]
async fn hide_overlay(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
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
        return Ok(win);
    }

    WebviewWindowBuilder::new(
        app_handle,
        "results",
        WebviewUrl::App("results.html".into()),
    )
    .title("DeskLens — Visual Search")
    .inner_size(520.0, 780.0)
    .always_on_top(true)
    .decorations(true)
    .visible(true)
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

    // 3. Ensure Results side-panel exists, show it, and broadcast payload
    let results_win = ensure_results_window(&app_handle)?;
    let _ = results_win.show();
    let _ = results_win.set_focus();

    if let Some(payload) = payload_to_send {
        let _ = app_handle.emit("load-search-payload", &payload);
    }

    println!("🔍 [DeskLens] Visual Search side-panel opened successfully.");
    Ok(())
}

/// Spawns a local one-shot HTTP bridge on 127.0.0.1:<port> that serves an auto-submitting
/// HTML form directly inside the user's default browser.
/// This prevents Windows `rundll32.exe` from truncating 450+ char Google Lens URLs at MAX_PATH (260 chars)
/// and ensures the upload uses the browser's own cookies and session.
#[tauri::command]
async fn search_google_lens(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let (b64_png, width, height) = {
        let guard = state
            .latest_crop_image
            .lock()
            .map_err(|e| format!("Failed to lock crop image state: {e}"))?;

        let image = guard
            .as_ref()
            .ok_or_else(|| "No cropped image available".to_string())?;

        let mut buffer = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {e}"))?;

        let encoded = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());
        (encoded, image.width(), image.height())
    };

    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(1729000000000);

    let html_body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>DeskLens — Uploading to Google Lens...</title>
  <style>
    body {{
      background: #121215;
      color: #f4f4f5;
      font-family: Inter, system-ui, -apple-system, sans-serif;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      height: 100vh;
      margin: 0;
    }}
    .card {{
      background: #18181b;
      border: 1px solid #27272a;
      border-radius: 12px;
      padding: 24px 32px;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 14px;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    }}
    img {{
      max-width: 280px;
      max-height: 200px;
      border-radius: 8px;
      border: 1px solid rgba(59, 130, 246, 0.5);
      object-fit: contain;
    }}
    .spinner {{
      color: #60a5fa;
      font-weight: 600;
      font-size: 0.95rem;
    }}
    .meta {{
      color: #a1a1aa;
      font-size: 0.8rem;
      font-family: monospace;
    }}
  </style>
</head>
<body>
  <div class="card">
    <img src="data:image/png;base64,{b64_png}" alt="Cropped region" />
    <div class="spinner">🔍 Uploading cropped region to Google Lens...</div>
    <div class="meta">{width} × {height} px</div>
  </div>
  <form id="lens-form" action="https://lens.google.com/v3/upload?ep=ccm&s=csp&st={timestamp_ms}" method="POST" enctype="multipart/form-data" style="display:none;">
    <input id="file-input" type="file" name="encoded_image" />
  </form>
  <script>
    (function() {{
      const b64 = "{b64_png}";
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) {{
        bytes[i] = bin.charCodeAt(i);
      }}
      const file = new File([bytes], "crop.png", {{ type: "image/png" }});
      const dt = new DataTransfer();
      dt.items.add(file);
      document.getElementById("file-input").files = dt.files;
      document.getElementById("lens-form").submit();
    }})();
  </script>
</body>
</html>"#
    );

    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind local Lens bridge server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to read local port: {e}"))?
        .port();

    let local_url = format!("http://127.0.0.1:{port}/lens");
    println!("🌐 [DeskLens] Launching browser via local Lens bridge: {local_url}");

    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let _ = listener.set_nonblocking(false);
        let start = std::time::Instant::now();

        for stream in listener.incoming().flatten() {
            if start.elapsed().as_secs() > 15 {
                break;
            }
            let mut stream = stream;
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
            let mut req_buf = [0u8; 2048];
            let n = stream.read(&mut req_buf).unwrap_or(0);
            if n == 0 {
                continue;
            }
            let req_str = String::from_utf8_lossy(&req_buf[..n]);
            if req_str.starts_with("GET /favicon") {
                let resp = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
                continue;
            }
            if req_str.starts_with("GET ") {
                let body_bytes = html_body.as_bytes();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body_bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body_bytes);
                let _ = stream.flush();
                break;
            }
        }
    });

    open_external_url(&local_url)?;
    Ok(local_url)
}

/// Copies the cropped image to the system clipboard and opens the chosen visual search provider in the default browser.
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
        "bing-visual" => "https://www.bing.com/visualsearch",
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

    Ok(path_str)
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
            close_results_window
        ])
        .setup(|app| {
            // On macOS, hide dock icon (Accessory mode)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // =========================================================================
            // SYSTEM TRAY: Interactive Menu (Settings & Quit)
            // =========================================================================
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

            let tray_menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&tray_menu)
                .tooltip("DeskLens — Google Lens for Desktop")
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        println!("⚙️ [DeskLens] Opening Settings window...");
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.set_fullscreen(false);
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

            if let Some(default_icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(default_icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            // =========================================================================
            // GLOBAL HOTKEY: Ctrl + Shift + X (Capture & Overlay)
            // =========================================================================
            let ctrl_shift_x = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::KeyX,
            );

            let app_handle = app.handle().clone();

            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |_app, shortcut, event| {
                        if shortcut == &ctrl_shift_x && event.state() == ShortcutState::Pressed {
                            println!(
                                "🎯 [DeskLens] Global shortcut (Ctrl + Shift + X) triggered! Capturing screen..."
                            );

                            if let Some(window) = app_handle.get_webview_window("main") {
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

                                            // 3. Make window fullscreen, always on top and visible
                                            let _ = window.set_fullscreen(true);
                                            let _ = window.set_always_on_top(true);
                                            let _ = window.show();
                                            let _ = window.set_focus();
                                            println!(
                                                "✨ [DeskLens] Screen captured and overlay active."
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!("❌ [DeskLens] Error capturing screen: {err}");
                                    }
                                }
                            }
                        }
                    })
                    .build(),
            )?;

            app.global_shortcut().register(ctrl_shift_x)?;
            println!("🚀 [DeskLens] Invisible engine started in system tray. Waiting for Ctrl + Shift + X...");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error while running DeskLens Tauri application");
}
