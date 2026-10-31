<div align="center">

# 🔎 DeskLens

**The power of Google Lens, natively on your Desktop.**  
*Lightweight background engine, instant OCR, on-screen translation, and visual search for Windows & Linux.*

[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8D8?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-2021-DEA584?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux-brightgreen?style=for-the-badge)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)

</div>

---

## 💡 About DeskLens

**DeskLens** is an open-source, ultra-lightweight desktop utility (~20 MB RAM) that brings the Google Lens mobile experience to Windows and Linux. 

With a single global keyboard shortcut, DeskLens freezes your screen and lets you select any region to:
1. **Search visually** on Google Lens in your browser with zero latency.
2. **Translate text on-screen** directly over the image, in any language pair, without covering or obscuring your view.
3. **Copy text from anywhere** (images, video streams, protected documents, game interfaces) with interactive word-level and line-level selection.

---

## ✨ Key Features

* 🚀 **Invisible Background Engine:** Sits quietly in your system tray near the clock without cluttering your taskbar.
* ⌨️ **Global Shortcut (`Ctrl + Shift + X`):** Instantly trigger screen capture from any application, game, or desktop environment.
* 📸 **Smart Screen Freezing:** Captures your active display instantly with smooth dimming, keeping your workflow uninterrupted.
* ✂️ **Precision Selection & Cropping:** Drag-to-select with interactive edge handles and live dimension feedback.
* 🔍 **Instant Google Lens Visual Search:** One-click image upload pipeline that opens full visual search in your default browser with immediate focus.
* 🌐 **On-Screen Multilingual Translation:**
  * Translates text directly over the original image with clean bounding boxes.
  * **Auto-detection** on the source language and automatic detection of your operating system's language on the target.
  * Switch between English, Portuguese, Spanish, French, German, Japanese, Chinese, Italian, Russian, Korean, and more.
  * Anti-collision layout algorithm prevents translated lines from overlapping each other.
  * Click any translated box to copy that phrase, or copy the entire translation at once.
* 📝 **High-Accuracy Native OCR Engine:**
  * **Windows:** Powered by the built-in Microsoft WinRT OCR engine (`Windows.Media.Ocr`) with zero external dependencies.
  * **Linux:** Powered by Tesseract OCR with 2x Lanczos super-resolution preprocessing to detect even tiny on-screen fonts.
  * **Interactive Word Highlighting:** Click a word to copy it, drag across words to select phrases, or double-click to copy an entire line.

---

## 📥 Download & Installation

Pre-compiled, ready-to-use binaries are available on the [**GitHub Releases**](https://github.com/GustavoGeminiano/desklens/releases) page.

### 🪟 Windows (Windows 10 / 11)
Download the latest `.exe` (NSIS Installer) or `.msi` package from [Releases](https://github.com/GustavoGeminiano/desklens/releases).  
Double-click the installer and launch DeskLens. It will run in your system tray with full native OCR out of the box.

### 🐧 Linux (Ubuntu, Debian, Fedora, Arch, Mint, etc.)
* **AppImage (Universal Portable):**
  ```bash
  chmod +x DeskLens_*.AppImage
  ./DeskLens_*.AppImage
  ```
* **DEB Package (Ubuntu / Debian):**
  ```bash
  sudo dpkg -i desklens_*_amd64.deb
  ```

> **Linux OCR Requirement:** On Linux, text recognition uses Tesseract. If you haven't installed language packs yet, run:
> ```bash
> sudo apt install tesseract-ocr tesseract-ocr-por tesseract-ocr-eng wl-clipboard
> ```

---

## ⌨️ Shortcuts & Controls

| Action | Control |
| :--- | :--- |
| **Trigger DeskLens** | <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>X</kbd> |
| **Select Region** | Click & drag mouse |
| **Adjust Selection** | Drag selection border handles |
| **Search on Google Lens** | Click **Search Image** |
| **Translate on Screen** | Click **Translate** |
| **Switch Languages** | Choose source / target in the language bar |
| **Copy Translated Text** | Click any translated box or **Copy Translation** |
| **Copy Word / Phrase** | Click a word pill or drag across words |
| **Copy Full Line** | Double-click on any word pill |
| **Exit Overlay / Clear** | <kbd>Esc</kbd> or click **Clear** / **✕** |

---

## 🛠️ Tech Stack

* **Core & Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://tauri.app/) (high performance, memory safety, tiny binary footprint).
* **Display Server & Capture:** `xcap` crate (multi-monitor Wayland & X11 capture, Windows desktop capture).
* **Frontend:** Vanilla JavaScript, Modern CSS3, HTML5 (rendered via WebKitGTK on Linux and WebView2 on Windows).
* **OCR Engines:**
  * Windows: `Windows.Media.Ocr` (built-in Windows runtime).
  * Linux: `tesseract-ocr` with automated 2x super-resolution scaling.
* **Continuous Integration:** GitHub Actions automated cross-platform build matrix.

---

## 💻 Running from Source

### Prerequisites
* [Node.js](https://nodejs.org/) (v18+)
* [Rust](https://www.rust-lang.org/tools/install) (v1.78+)
* **Linux Build Dependencies (Ubuntu / Debian):**
  ```bash
  sudo apt update
  sudo apt install -y \
    build-essential \
    curl \
    wget \
    file \
    pkg-config \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libxdo-dev \
    libpipewire-0.3-dev \
    libspa-0.2-dev \
    tesseract-ocr \
    tesseract-ocr-por \
    tesseract-ocr-eng \
    wl-clipboard \
    xclip
  ```

### Build & Run

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
   * **Linux:**
     ```bash
     ./run-linux.sh
     ```
   * **Windows:**
     ```bash
     npm run tauri dev
     ```

4. Build production installer packages locally:
   ```bash
   npm run tauri build
   ```
   Installers will be generated under `src-tauri/target/release/bundle/`.

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.

<div align="center">
Developed with ❤️ by <strong>Gustavo Geminiano</strong>
</div>
