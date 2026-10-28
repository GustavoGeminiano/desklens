<div align="center">

# 🔎 DeskLens

**The power of Google Lens, natively on your Desktop.**  
*Lightweight background engine, instant OCR, and visual search for Windows and Linux.*

[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-2021-DEA584?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-brightgreen?style=for-the-badge)]()

</div>

---

## 💡 About DeskLens

**DeskLens** is an ultra-lightweight desktop utility designed to run silently in the background, consuming minimal system resources (~20 MB RAM). With a simple global keyboard shortcut, DeskLens freezes the current screen and transforms any visible area into selectable text or an instant visual search on Google Lens — directly inside an integrated app window, without cluttering your browser with extra tabs.

---

## ✨ Features

* 🚀 **Invisible Engine & System Tray:** Runs silently in the system tray without taking up space on your main taskbar.
* ⌨️ **Global Shortcut:** Instantly invoked from anywhere via global hotkey (`Ctrl + Shift + X`).
* 📸 **Smart Screen Freezing:** Captures your active display with an intuitive overlay without interrupting your workflow.
* ✂️ **Dynamic Cropping:** Drag-and-drop selection handles to focus on any image, code snippet, or UI element.
* 🔍 **Embedded Visual Search:** Reverse image search powered by Google Lens inside a native WebView panel.
* 📝 **OCR & Text Extraction:** Optical character recognition directly extracts text from images or video frames straight into your clipboard.

---

## 🛠️ Tech Stack

* **Core & Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://tauri.app/) (high performance, memory safety, minimal footprint).
* **Interface (Frontend):** Modern HTML5, CSS3, and Vanilla JavaScript (rendered via native system WebView2).
* **System Integration:** Official Tauri v2 plugins (`tray-icon` and `global-shortcut`).

---

## 🗺️ Implementation Roadmap

- [x] **Sprint 0: Foundation & Invisible Engine**
  - [x] Tauri v2 setup (Rust + Vanilla JS).
  - [x] Window hidden from main taskbar on startup (`skipTaskbar`).
  - [x] Native System Tray menu (*Settings* and *Quit*).
  - [x] Global system hotkey registration (`Ctrl + Shift + X`).
- [ ] **Sprint 1: Screen Capture & Overlay**
  - [ ] Fullscreen screen capture upon hotkey trigger.
  - [ ] Frameless, always-on-top overlay display.
  - [ ] Frozen frame canvas with visual border cues.
- [ ] **Sprint 2: Selection & Cropping Tool**
  - [ ] Mouse selection handlers with dimmed outer mask.
  - [ ] Bounding box coordinates dispatched to Rust backend.
- [ ] **Sprint 3: Integrated Visual Search**
  - [ ] Embedded side-panel WebView for search results.
  - [ ] Image slice payload dispatched to Google Lens.
- [ ] **Sprint 4: OCR & Text Extraction**
  - [ ] Optical character recognition engine integration.
  - [ ] Automatic system clipboard copy.

---

## 🚀 Getting Started

### Prerequisites
* [Node.js](https://nodejs.org/) (v18+)
* [Rust](https://www.rust-lang.org/tools/install) (v1.78+)
* **Windows:** Visual Studio 2022 C++ Build Tools (with *"Desktop development with C++"* workload)

### Installation & Run

1. Clone the repository:
```bash
git clone https://github.com/GustavoGeminiano/desklens.git
cd desklens
```

2. Install dependencies:
```bash
npm install
```

3. Run in development mode:
```bash
npm run tauri dev
```

4. **Testing the application:**
   * The app will start silently in your system tray (near the clock).
   * Press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>X</kbd> anywhere to test the global hotkey trigger.
   * Right-click the tray icon to open *Settings* or *Quit*.

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.

---

<div align="center">
Developed by <strong>Gustavo Geminiano</strong>
</div>
