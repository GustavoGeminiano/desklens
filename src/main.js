// DeskLens — Frontend Controller (Tauri v2)

window.addEventListener("DOMContentLoaded", async () => {
  const overlayView = document.getElementById("overlay-view");
  const settingsView = document.getElementById("settings-view");
  const frozenImage = document.getElementById("frozen-image");
  const closeOverlayBtn = document.getElementById("close-overlay-btn");
  const hideBtn = document.getElementById("hide-btn");
  const testCaptureBtn = document.getElementById("test-capture-btn");

  // Helper to switch view modes
  function setViewMode(mode) {
    if (mode === "overlay") {
      overlayView.classList.remove("hidden");
      settingsView.classList.add("hidden");
    } else {
      overlayView.classList.add("hidden");
      settingsView.classList.remove("hidden");
    }
  }

  // Dismiss overlay and return to hidden background mode
  async function hideOverlay() {
    setViewMode("settings");
    if (window.__TAURI__?.core?.invoke) {
      await window.__TAURI__.core.invoke("hide_overlay");
    } else if (window.__TAURI__?.window) {
      const currentWindow = window.__TAURI__.window.getCurrentWindow();
      await currentWindow.setFullscreen(false);
      await currentWindow.hide();
    }
  }

  // 1. Listen for Escape key to exit overlay
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hideOverlay();
    }
  });

  // 2. Close overlay button
  if (closeOverlayBtn) {
    closeOverlayBtn.addEventListener("click", () => {
      hideOverlay();
    });
  }

  // 3. Hide button on Settings panel
  if (hideBtn) {
    hideBtn.addEventListener("click", () => {
      hideOverlay();
    });
  }

  // 4. Test capture button on Settings panel
  if (testCaptureBtn) {
    testCaptureBtn.addEventListener("click", async () => {
      if (window.__TAURI__?.core?.invoke) {
        try {
          const screenshotUrl = await window.__TAURI__.core.invoke("trigger_capture");
          if (frozenImage && screenshotUrl) {
            frozenImage.src = screenshotUrl;
            setViewMode("overlay");
            if (window.__TAURI__?.window) {
              const currentWindow = window.__TAURI__.window.getCurrentWindow();
              await currentWindow.setFullscreen(true);
            }
          }
        } catch (err) {
          console.error("Failed to capture screenshot:", err);
        }
      }
    });
  }

  // 5. Setup Tauri v2 Event Listeners (if running inside Tauri runtime)
  if (window.__TAURI__?.event?.listen) {
    // Received new screenshot from Rust backend
    await window.__TAURI__.event.listen("screenshot-captured", (event) => {
      if (frozenImage && event.payload) {
        frozenImage.src = event.payload;
        setViewMode("overlay");
      }
    });

    // Received view mode change
    await window.__TAURI__.event.listen("view-mode", (event) => {
      if (event.payload === "overlay") {
        setViewMode("overlay");
      } else {
        setViewMode("settings");
      }
    });
  }
});
