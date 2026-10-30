// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--trigger" || a == "-t" || a == "trigger") {
        #[cfg(target_os = "linux")]
        {
            use std::io::Write;
            use std::os::unix::net::UnixStream;
            let uid = unsafe { libc::getuid() };
            let socket_path = format!("/tmp/desklens_{}.sock", uid);
            if let Ok(mut stream) = UnixStream::connect(&socket_path) {
                let _ = stream.write_all(b"trigger\n");
                let _ = stream.flush();
                println!("🎯 [DeskLens] Sent trigger signal to active instance.");
                return;
            }
        }
    }

    desklens_lib::run()
}
