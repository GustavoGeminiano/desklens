use desklens_lib::LinuxDisplayInfo;

#[test]
fn test_linux_display_info_detect_host() {
    let info = LinuxDisplayInfo::detect();
    println!("Detected host display info: {:?}", info);
    // Ensure log_startup does not panic
    info.log_startup();

    // Verify consistency
    if info.session_type.eq_ignore_ascii_case("wayland") || info.wayland_display.is_some() {
        assert!(info.is_wayland(), "Should be detected as Wayland");
    }
}

#[test]
fn test_linux_display_info_simulated_states() {
    // We test the logic of LinuxDisplayInfo fields directly
    // Case 1: Pure Wayland
    let wayland_pure = LinuxDisplayInfo {
        session_type: "wayland".to_string(),
        wayland_display: Some("wayland-0".to_string()),
        x11_display: Some(":0".to_string()),
        gdk_backend: None,
        desktop: Some("ubuntu:GNOME".to_string()),
    };
    assert!(wayland_pure.is_wayland());
    wayland_pure.log_startup();

    // Case 2: Mixed-case Wayland string
    let wayland_mixed = LinuxDisplayInfo {
        session_type: "WaYlAnD".to_string(),
        wayland_display: None,
        x11_display: None,
        gdk_backend: None,
        desktop: None,
    };
    assert!(wayland_mixed.is_wayland(), "is_wayland must be case-insensitive");

    // Case 3: Empty XDG_SESSION_TYPE but WAYLAND_DISPLAY present
    let wayland_from_socket = LinuxDisplayInfo {
        session_type: "".to_string(),
        wayland_display: Some("wayland-1".to_string()),
        x11_display: None,
        gdk_backend: None,
        desktop: None,
    };
    assert!(wayland_from_socket.is_wayland(), "WAYLAND_DISPLAY socket should trigger is_wayland");

    // Case 4: Pure X11
    let x11_session = LinuxDisplayInfo {
        session_type: "x11".to_string(),
        wayland_display: None,
        x11_display: Some(":0".to_string()),
        gdk_backend: Some("x11".to_string()),
        desktop: Some("XFCE".to_string()),
    };
    assert!(!x11_session.is_wayland(), "Pure X11 must NOT be detected as Wayland");
    x11_session.log_startup();

    // Case 5: Unset / unknown environment
    let unknown_session = LinuxDisplayInfo {
        session_type: "unknown".to_string(),
        wayland_display: None,
        x11_display: None,
        gdk_backend: None,
        desktop: None,
    };
    assert!(!unknown_session.is_wayland());
    unknown_session.log_startup();

    // Case 6: Wayland session with GDK_BACKEND=x11 (run-linux.sh mode)
    let xwayland_forced = LinuxDisplayInfo {
        session_type: "wayland".to_string(),
        wayland_display: Some("wayland-0".to_string()),
        x11_display: Some(":0".to_string()),
        gdk_backend: Some("x11".to_string()),
        desktop: Some("ubuntu:GNOME".to_string()),
    };
    assert!(xwayland_forced.is_wayland());
    assert_eq!(xwayland_forced.gdk_backend.as_deref(), Some("x11"));
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct MockMonitor {
    id: usize,
    primary_res: Result<bool, String>,
}

impl MockMonitor {
    fn is_primary(&self) -> Result<bool, String> {
        self.primary_res.clone()
    }
}

/// Exact replica of monitor selection logic in capture_screen_raw:
/// let monitor = monitors
///     .iter()
///     .find(|m| m.is_primary().unwrap_or(false))
///     .unwrap_or(&monitors[0]);
fn select_mock_monitor<'a>(monitors: &'a [MockMonitor]) -> Result<&'a MockMonitor, &'static str> {
    if monitors.is_empty() {
        return Err("No active monitor found on system");
    }
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .unwrap_or(&monitors[0]);
    Ok(monitor)
}

#[test]
fn test_capture_screen_monitor_selection_edge_cases() {
    // 1. Empty monitors list
    let empty: Vec<MockMonitor> = vec![];
    let res = select_mock_monitor(&empty);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "No active monitor found on system");

    // 2. Single monitor, is_primary = true
    let single_primary = vec![MockMonitor { id: 1, primary_res: Ok(true) }];
    assert_eq!(select_mock_monitor(&single_primary).unwrap().id, 1);

    // 3. Single monitor, is_primary = false (typical Xwayland headless/virtual output)
    let single_non_primary = vec![MockMonitor { id: 2, primary_res: Ok(false) }];
    assert_eq!(select_mock_monitor(&single_non_primary).unwrap().id, 2);

    // 4. Single monitor, is_primary returns Err
    let single_err = vec![MockMonitor { id: 3, primary_res: Err("XCB query failed".to_string()) }];
    assert_eq!(select_mock_monitor(&single_err).unwrap().id, 3);

    // 5. Multi-monitor: None is primary (all false)
    let multi_no_primary = vec![
        MockMonitor { id: 10, primary_res: Ok(false) },
        MockMonitor { id: 20, primary_res: Ok(false) },
        MockMonitor { id: 30, primary_res: Ok(false) },
    ];
    // Must fall back to first monitor (&monitors[0])
    assert_eq!(select_mock_monitor(&multi_no_primary).unwrap().id, 10);

    // 6. Multi-monitor: Second monitor is primary
    let multi_second_primary = vec![
        MockMonitor { id: 10, primary_res: Ok(false) },
        MockMonitor { id: 20, primary_res: Ok(true) },
        MockMonitor { id: 30, primary_res: Ok(false) },
    ];
    assert_eq!(select_mock_monitor(&multi_second_primary).unwrap().id, 20);

    // 7. Multi-monitor: Third monitor is primary, others error
    let multi_third_primary_with_err = vec![
        MockMonitor { id: 10, primary_res: Err("Monitor disconnected".to_string()) },
        MockMonitor { id: 20, primary_res: Ok(false) },
        MockMonitor { id: 30, primary_res: Ok(true) },
    ];
    assert_eq!(select_mock_monitor(&multi_third_primary_with_err).unwrap().id, 30);

    // 8. Multi-monitor: All monitors return error on is_primary()
    let multi_all_err = vec![
        MockMonitor { id: 100, primary_res: Err("err1".to_string()) },
        MockMonitor { id: 200, primary_res: Err("err2".to_string()) },
    ];
    assert_eq!(select_mock_monitor(&multi_all_err).unwrap().id, 100);
}

#[test]
fn test_real_xcap_query_on_host() {
    match xcap::Monitor::all() {
        Ok(monitors) => {
            println!("Host reported {} monitor(s).", monitors.len());
            for (idx, m) in monitors.iter().enumerate() {
                let name = m.name().unwrap_or_else(|_| "unknown".to_string());
                let primary = m.is_primary().unwrap_or(false);
                let width = m.width().unwrap_or(0);
                let height = m.height().unwrap_or(0);
                println!("Monitor #{idx}: name='{name}', primary={primary}, resolution={width}x{height}");
            }
            if !monitors.is_empty() {
                let chosen = monitors
                    .iter()
                    .find(|m| m.is_primary().unwrap_or(false))
                    .unwrap_or(&monitors[0]);
                println!("Chosen monitor for capture: {:?}", chosen.name());

                // Try capture_image on host monitor
                match chosen.capture_image() {
                    Ok(img) => {
                        println!("Screen capture succeeded! Dimensions: {}x{}", img.width(), img.height());
                        assert!(img.width() > 0 && img.height() > 0);
                    }
                    Err(e) => {
                        println!("Screen capture returned error on this host environment: {e}");
                        // Under headless CI or certain sandboxes, xcap capture might return an error, which should not panic
                    }
                }
            }
        }
        Err(e) => {
            println!("xcap::Monitor::all() returned error: {e}");
        }
    }
}
