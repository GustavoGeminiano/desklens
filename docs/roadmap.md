# DeskLens — Implementation Roadmap

This document structures the development of DeskLens into actionable, incremental sprints.

---

### Sprint 0: Foundation & Invisible Engine ✅
**Goal:** Initialize the base project, keep it running silently in the system tray, and listen for global keyboard shortcuts.

- [x] **Step 1:** Initialize base project with Tauri v2 (Rust backend + Vanilla HTML/CSS/JS frontend).
- [x] **Step 2:** Configure `tauri.conf.json` to hide window and taskbar icon on launch (`skipTaskbar: true`).
- [x] **Step 3:** Implement native System Tray icon in Rust with interactive menu (`Settings` and `Quit`).
- [x] **Step 4:** Register global shortcut (`Ctrl + Shift + X`) in Rust and verify background activation.

---

### Sprint 1: Screen Capture & Overlay (Visual Experience) ✅
**Goal:** Freeze the active screen and display the DeskLens capture interface.

- [x] **Step 1:** Capture full-resolution screenshot across active monitors upon shortcut trigger (using `xcap` crate in Rust).
- [x] **Step 2:** Encode the captured frame (or store in temporary cache) and pass to frontend.
- [x] **Step 3:** Display the main Tauri window in fullscreen, frameless, and always-on-top mode.
- [x] **Step 4:** Render the frozen frame as a static background with animated decorative borders indicating active capture mode.
- [x] **Step 5:** Allow immediate dismiss on <kbd>ESC</kbd> or close button to return to silent background.

---

### Sprint 2: Selection & Cropping Tool ✅
**Goal:** Allow users to drag-select regions on screen for search or text extraction.

- [x] **Step 1:** Implement mouse event handlers (`mousedown`, `mousemove`, `mouseup`) with dynamic rectangle tracking.
- [x] **Step 2:** Render visual selection box with darkened SVG cut-out outer mask and Google Lens corner brackets.
- [x] **Step 3:** Implement draggable edge handles allowing users to expand or shrink the crop box boundaries.
- [x] **Step 4:** Send coordinates to Rust backend command (`crop_selection`) with display DPI scaling to crop the exact image slice.
- [x] **Step 5:** Display floating action bar with instant cropped thumbnail preview, Google Lens search action, and OCR copy action.

---

### Sprint 3: Integrated Visual Search
**Goal:** Perform reverse image lookup and display results inside a dedicated application WebView.

- [ ] **Step 1:** Open a compact side-panel window upon completing selection.
- [ ] **Step 2:** Prepare cropped image payload and dispatch query to Google Lens web endpoint.
- [ ] **Step 3:** Render query results inside the embedded WebView without external browser dependencies.

---

### Sprint 4: Optical Character Recognition (OCR)
**Goal:** Extract text from screen crops with instant clipboard integration.

- [ ] **Step 1:** Add floating "Copy Text" quick action bar near active selection.
- [ ] **Step 2:** Integrate local OCR engine (e.g. Tesseract / Windows Media OCR API) for offline text extraction.
- [ ] **Step 3:** (Optional) Provide configuration settings for cloud-assisted OCR (Gemini / Cloud Vision API).
- [ ] **Step 4:** Copy extracted text directly to system clipboard.
