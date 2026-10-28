use base64::Engine;
use image::RgbaImage;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Application state storing the latest captured raw screenshot in memory for fast cropping.
pub struct AppState {
    pub current_screenshot: Mutex<Option<RgbaImage>>,
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

/// Closes the fullscreen overlay and hides the window back to the background.
#[tauri::command]
fn hide_overlay(window: tauri::Window) {
    let _ = window.set_fullscreen(false);
    let _ = window.hide();
}

/// Allows the frontend to manually trigger a fresh screenshot.
#[tauri::command]
fn trigger_capture(app_handle: tauri::AppHandle) -> Result<String, String> {
    let raw_image = capture_screen_raw()?;
    let data_url = encode_png_data_url(&raw_image)?;

    // Store in state for subsequent crops
    if let Some(state) = app_handle.try_state::<AppState>() {
        if let Ok(mut lock) = state.current_screenshot.lock() {
            *lock = Some(raw_image);
        }
    }

    Ok(data_url)
}

/// Crops a user-selected rectangle from the current screenshot and returns the cropped slice as PNG data URL.
#[tauri::command]
fn crop_selection(
    state: tauri::State<'_, AppState>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    screen_width: u32,
    screen_height: u32,
) -> Result<String, String> {
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

    let cropped = image::imageops::crop_imm(image, real_x, real_y, clamped_w, clamped_h).to_image();
    println!("✂️ [DeskLens] Successfully cropped region: {clamped_w}x{clamped_h} px (at {real_x}, {real_y})");

    encode_png_data_url(&cropped)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            current_screenshot: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            hide_overlay,
            trigger_capture,
            crop_selection
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
                            let _ = window.emit("view-mode", "settings");
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

                                            // 2. Dispatch captured image to frontend
                                            let _ = window.emit("screenshot-captured", &data_url);
                                            let _ = window.emit("view-mode", "overlay");

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
