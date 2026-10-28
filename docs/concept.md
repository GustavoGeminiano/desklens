# DeskLens — Project Concept

## Overview

DeskLens is a lightweight background desktop utility for Windows and Linux inspired by Google Lens. Activated via a global keyboard shortcut, it freezes the screen, overlays an interactive border, and allows users to interact with visual elements by extracting text (OCR) or performing reverse image searches directly without opening an external browser tab.

## Key Features

* **Extract & Search Text from Images:** High-accuracy OCR to convert any screen area into selectable, searchable, and copyable text.
* **Visual Search (Google Lens):** Crop any image on screen and send it directly to Google Lens / visual search within an integrated in-app WebView.
* **Dynamic Crop & Resize:** Resize and adjust the capture box with intuitive drag-and-drop handles to focus on specific regions.
* **On-Screen Translation (Planned):** Automated real-time translation of detected on-screen text.

## Technical Architecture

* **Engine (Rust + Tauri v2):** Runs invisibly in the system tray with minimal memory footprint (~20 MB idle). Listens for global hotkeys (`Ctrl + Shift + X`).
* **Overlay Interface (HTML/CSS/JS):** Frameless, always-on-top transparent window that renders the frozen screen canvas and interactive selection tools.
* **Results Panel:** Compact embedded WebView side-panel displaying search and OCR results seamlessly.
