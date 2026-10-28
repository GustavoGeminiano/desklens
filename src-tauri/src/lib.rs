use base64::Engine;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Captures the primary display screen using xcap and returns it as a Base64-encoded PNG Data URL.
fn capture_screen() -> Result<String, String> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| xcap::Monitor::all().ok()?.into_iter().next())
        .ok_or_else(|| "No active monitor found".to_string())?;

    let image = monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture screen image: {e}"))?;

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

/// Allows the frontend to manually request a fresh screenshot.
#[tauri::command]
fn trigger_capture() -> Result<String, String> {
    capture_screen()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![hide_overlay, trigger_capture])
        .setup(|app| {
            // On macOS, hide dock icon (Accessory mode)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // =========================================================================
            // STEP 3: System Tray with Interactive Menu
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
            // SPRINT 1: Global Hotkey & Screen Capture Overlay
            // =========================================================================
            let ctrl_shift_x = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::KeyX,
            );

            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |app_handle, shortcut, event| {
                        if shortcut == &ctrl_shift_x && event.state() == ShortcutState::Pressed {
                            println!(
                                "🎯 [DeskLens] Global shortcut (Ctrl + Shift + X) triggered! Capturing screen..."
                            );

                            if let Some(window) = app_handle.get_webview_window("main") {
                                // 1. Capture screen before opening window
                                match capture_screen() {
                                    Ok(screenshot_url) => {
                                        // 2. Dispatch captured image to frontend
                                        let _ = window.emit("screenshot-captured", &screenshot_url);
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
