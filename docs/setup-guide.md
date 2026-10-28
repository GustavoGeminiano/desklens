# DeskLens — Complete Setup & Environment Guide

This document is a comprehensive technical guide detailing everything built so far (Sprint 0) and how to replicate the development environment on any other machine (Windows/Linux) from scratch.

---

## 📌 1. Project Overview & Current State

### What is DeskLens?
DeskLens is an ultra-lightweight desktop utility inspired by Google Lens. It runs silently in the background, consuming minimal system resources (~20 MB RAM), listening for a global shortcut to freeze the screen and provide instant OCR and visual search capabilities.

### What was completed in Sprint 0:
* **Tauri v2 Initialization:** Complete project skeleton with Rust backend and Vanilla JS/HTML5/CSS3 frontend.
* **Invisible Startup:** Configured `tauri.conf.json` with `visible: false` and `skipTaskbar: true` so the app starts 100% hidden in the background without taskbar clutter.
* **System Tray Integration:** Built a native Windows/Linux system tray icon near the system clock with an interactive menu containing:
  * **Settings:** Opens and focuses the preferences/control window.
  * **Quit:** Gracefully terminates the application (`exit(0)`).
* **Global Hotkey Listener:** Integrated `tauri-plugin-global-shortcut` v2 to listen for `Ctrl + Shift + X` from anywhere in the OS, firing on `ShortcutState::Pressed`.
* **Repository Architecture:** Clean, modular structure with `docs/`, `src/`, `src-tauri/`, and automated `.gitignore`.

---

## ⚙️ 2. Machine Prerequisites

To build and run DeskLens on a new machine, install these 3 core requirements:

### 1. Node.js (v18+)
Download from [nodejs.org](https://nodejs.org/) or install via terminal:
```powershell
# Windows (winget)
winget install OpenJS.NodeJS.LTS
```

### 2. Rust & Cargo Toolchain
Download from [rustup.rs](https://rustup.rs/) or install via terminal:
```powershell
# Windows (winget)
winget install Rustlang.Rustup -e --silent
```
*Note: Make sure `%USERPROFILE%\.cargo\bin` is added to your environment `PATH`.*

### 3. C++ Build Tools & Linker (CRITICAL FOR WINDOWS)
> ⚠️ **Important:** Visual Studio Code alone is **NOT** sufficient. Rust on Windows requires the official Microsoft C++ linker (`link.exe`) and the Windows SDK.

**Option A (Terminal):**
Run PowerShell as Administrator:
```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --silent
```

**Option B (Visual Studio Installer GUI):**
1. Download [vs_buildtools.exe](https://visualstudio.microsoft.com/visual-cpp-build-tools/).
2. Run the installer and check the workload: **"Desktop development with C++"** (*Desenvolvimento para desktop com C++*).
3. Complete the installation.

---

## 🚀 3. Step-by-Step: Setting Up on a New Computer

Once the prerequisites are installed, clone and launch the project:

### Step 1: Clone the Repository
```powershell
git clone https://github.com/GustavoGeminiano/desklens.git
cd desklens
```

### Step 2: Install Frontend & Tauri Dependencies
```powershell
npm install
```

### Step 3: Run in Development Mode
```powershell
npm run tauri dev
```

### Step 4: Verify Everything Works
1. Look at your Windows notification area / system tray (bottom-right near the clock):
   * The **DeskLens** tray icon will appear.
   * Right-click the icon to test **Settings** (shows window) and **Quit** (closes app).
2. Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>X</kbd> anywhere on your desktop:
   * The terminal will immediately output:
   ```text
   🎯 [DeskLens] Global shortcut (Ctrl + Shift + X) triggered! Ready for screen capture.
   ```

---

## 📂 4. Project Structure Explained

```text
desklens/
├── .gitignore                   # Ignores node_modules/, Rust target/, and logs
├── LICENSE                      # MIT License
├── README.md                    # Public project showcase and sprint roadmap
├── package.json                 # Node dependencies & Tauri CLI script
├── docs/                        # Internal engineering documentation
│   ├── concept.md               # Vision, core capabilities, and architecture
│   ├── roadmap.md               # Sprints 0 to 4 breakdown
│   └── setup-guide.md           # This setup and replication guide
├── src/                         # Frontend Web Interface
│   ├── index.html               # Settings/Control UI
│   ├── styles.css               # Clean dark-mode stylesheet
│   └── main.js                  # Frontend window control via window.__TAURI__
└── src-tauri/                   # Rust Backend (The Invisible Engine)
    ├── Cargo.toml               # Rust dependencies: tauri v2 (tray-icon), global-shortcut
    ├── build.rs                 # Tauri build script
    ├── tauri.conf.json          # Window rules (hidden, skipTaskbar, alwaysOnTop)
    ├── capabilities/
    │   └── default.json         # Tauri v2 security ACL permissions (shortcuts, window)
    ├── icons/                   # System tray and application icons (ICO/PNG/ICNS)
    └── src/
        ├── main.rs              # Binary entrypoint (hides console window in release)
        └── lib.rs               # Engine logic: System Tray menu + Hotkey listener
```

---

## 🛠️ 5. Troubleshooting Common Issues

| Issue / Error | Root Cause | Solution |
| :--- | :--- | :--- |
| `error: linker link.exe not found` | Visual Studio C++ compiler is missing on Windows. | Install Visual Studio 2022 Build Tools with the **Desktop development with C++** workload. |
| `'cargo' is not recognized` | Cargo binary folder is not in the active shell's `PATH`. | Restart the terminal or run `$env:Path += ";$env:USERPROFILE\.cargo\bin"`. |
| `Could not read package.json: ENOENT` | Terminal is open in the wrong folder (e.g. `C:\Windows\System32`). | Run `cd <path_to_desklens>` before running npm commands. |
| Double hotkey triggers | Hotkey handler listening to both keydown and keyup. | Filter strictly by `event.state() == ShortcutState::Pressed` in `lib.rs` (already implemented). |

---

## 🎯 6. Next Milestone: Sprint 1 (Screen Capture & Overlay)

When you resume development on this or any other computer, the next tasks are:
1. Add the `xcap` crate to `src-tauri/Cargo.toml` for cross-platform desktop screenshots.
2. In `lib.rs`, trigger the screen capture immediately upon pressing `Ctrl + Shift + X`.
3. Pass the captured screenshot to the Tauri window and display it as an always-on-top, frameless, fullscreen canvas.
4. Add CSS decorative border cues to signal active capture mode.
