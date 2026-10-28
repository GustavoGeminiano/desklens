// DeskLens — Interactive Selection & Cropping Controller (Tauri v2)

window.addEventListener("DOMContentLoaded", async () => {
  // Views
  const overlayView = document.getElementById("overlay-view");
  const settingsView = document.getElementById("settings-view");
  const frozenImage = document.getElementById("frozen-image");
  const selectionLayer = document.getElementById("selection-layer");

  // Selection & Crop Elements
  const maskCutout = document.getElementById("mask-cutout");
  const cropBox = document.getElementById("crop-box");
  const cropDimensions = document.getElementById("crop-dimensions");
  const cropActions = document.getElementById("crop-actions");
  const cropPreviewThumb = document.getElementById("crop-preview-thumb");

  // Control Buttons
  const closeOverlayBtn = document.getElementById("close-overlay-btn");
  const hideBtn = document.getElementById("hide-btn");
  const testCaptureBtn = document.getElementById("test-capture-btn");
  const clearActionBtn = document.getElementById("clear-action-btn");
  const searchActionBtn = document.getElementById("search-action-btn");
  const ocrActionBtn = document.getElementById("ocr-action-btn");

  // State
  let isSelecting = false;
  let isResizing = false;
  let activeHandle = null;
  let startX = 0;
  let startY = 0;
  let currentRect = { x: 0, y: 0, w: 0, h: 0 };
  let currentCropDataUrl = null;

  // Helper to switch view modes
  function setViewMode(mode) {
    if (mode === "overlay") {
      overlayView.classList.remove("hidden");
      settingsView.classList.add("hidden");
    } else {
      clearSelection();
      overlayView.classList.add("hidden");
      settingsView.classList.remove("hidden");
    }
  }

  // Dismiss overlay and return to hidden background mode
  async function hideOverlay() {
    clearSelection();
    setViewMode("settings");
    if (window.__TAURI__?.core?.invoke) {
      await window.__TAURI__.core.invoke("hide_overlay");
    } else if (window.__TAURI__?.window) {
      const currentWindow = window.__TAURI__.window.getCurrentWindow();
      await currentWindow.setFullscreen(false);
      await currentWindow.hide();
    }
  }

  // Update visual crop box and SVG dimming mask
  function updateCropBox(x, y, w, h) {
    currentRect = { x, y, w, h };
    cropBox.style.left = `${x}px`;
    cropBox.style.top = `${y}px`;
    cropBox.style.width = `${w}px`;
    cropBox.style.height = `${h}px`;

    maskCutout.setAttribute("x", x);
    maskCutout.setAttribute("y", y);
    maskCutout.setAttribute("width", w);
    maskCutout.setAttribute("height", h);

    cropDimensions.textContent = `${Math.round(w)} × ${Math.round(h)} px`;
  }

  // Clear current active selection
  function clearSelection() {
    isSelecting = false;
    isResizing = false;
    activeHandle = null;
    currentRect = { x: 0, y: 0, w: 0, h: 0 };
    currentCropDataUrl = null;
    cropBox.classList.add("hidden");
    cropActions.classList.add("hidden");
    maskCutout.setAttribute("width", 0);
    maskCutout.setAttribute("height", 0);
  }

  // Request backend Rust to crop the selected frame slice
  async function requestBackendCrop() {
    if (currentRect.w < 20 || currentRect.h < 20) return;

    if (window.__TAURI__?.core?.invoke) {
      try {
        const croppedDataUrl = await window.__TAURI__.core.invoke("crop_selection", {
          x: Math.round(currentRect.x),
          y: Math.round(currentRect.y),
          width: Math.round(currentRect.w),
          height: Math.round(currentRect.h),
          screenWidth: window.innerWidth,
          screenHeight: window.innerHeight,
        });

        if (croppedDataUrl) {
          currentCropDataUrl = croppedDataUrl;
          if (cropPreviewThumb) {
            cropPreviewThumb.src = croppedDataUrl;
          }
        }
      } catch (err) {
        console.error("Failed to crop selection:", err);
      }
    }
  }

  // Position floating action toolbar near the selection
  function positionActionToolbar() {
    const spaceBelow = window.innerHeight - (currentRect.y + currentRect.h);
    if (spaceBelow < 65) {
      cropActions.style.bottom = "auto";
      cropActions.style.top = "-52px";
    } else {
      cropActions.style.top = "auto";
      cropActions.style.bottom = "-52px";
    }
    cropActions.classList.remove("hidden");
  }

  // =========================================================================
  // Mouse Drag & Resize Handlers (Sprint 2)
  // =========================================================================

  selectionLayer.addEventListener("mousedown", (e) => {
    // If clicking on floating action bar or buttons, do not restart selection
    if (e.target.closest("#crop-actions") || e.target.closest(".lens-header")) {
      return;
    }

    // Check if clicking on an edge handle to resize existing box
    const handleEl = e.target.closest(".edge-handle");
    if (handleEl && !cropBox.classList.contains("hidden")) {
      isResizing = true;
      activeHandle = handleEl.dataset.handle;
      startX = e.clientX;
      startY = e.clientY;
      cropActions.classList.add("hidden");
      e.stopPropagation();
      return;
    }

    // Start brand new drag selection
    isSelecting = true;
    startX = e.clientX;
    startY = e.clientY;
    cropActions.classList.add("hidden");
    cropBox.classList.remove("hidden");
    updateCropBox(startX, startY, 0, 0);
  });

  window.addEventListener("mousemove", (e) => {
    if (isSelecting) {
      const curX = e.clientX;
      const curY = e.clientY;

      const x = Math.min(startX, curX);
      const y = Math.min(startY, curY);
      const w = Math.abs(curX - startX);
      const h = Math.abs(curY - startY);

      updateCropBox(x, y, w, h);
    } else if (isResizing && activeHandle) {
      const curX = e.clientX;
      const curY = e.clientY;
      let { x, y, w, h } = currentRect;

      if (activeHandle === "top") {
        const newY = Math.min(curY, y + h - 15);
        h += y - newY;
        y = newY;
      } else if (activeHandle === "bottom") {
        h = Math.max(15, curY - y);
      } else if (activeHandle === "left") {
        const newX = Math.min(curX, x + w - 15);
        w += x - newX;
        x = newX;
      } else if (activeHandle === "right") {
        w = Math.max(15, curX - x);
      }

      updateCropBox(x, y, w, h);
    }
  });

  window.addEventListener("mouseup", async () => {
    if (isSelecting || isResizing) {
      isSelecting = false;
      isResizing = false;
      activeHandle = null;

      if (currentRect.w >= 20 && currentRect.h >= 20) {
        positionActionToolbar();
        await requestBackendCrop();
      } else {
        clearSelection();
      }
    }
  });

  // Action Buttons
  if (clearActionBtn) {
    clearActionBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      clearSelection();
    });
  }

  if (searchActionBtn) {
    searchActionBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      console.log("🔍 Search triggered with crop payload:", currentCropDataUrl?.slice(0, 50));
      // Sprint 3 will open the integrated side-panel webview with this cropped image
      alert("✨ Ready for Sprint 3 (Integrated Visual Search with Google Lens)!");
    });
  }

  if (ocrActionBtn) {
    ocrActionBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      console.log("📝 OCR copy triggered with crop payload:", currentCropDataUrl?.slice(0, 50));
      // Sprint 4 will run OCR and copy directly to system clipboard
      alert("📝 Ready for Sprint 4 (OCR & Text Extraction)!");
    });
  }

  // =========================================================================
  // Keyboard & Overlay Exit Controls
  // =========================================================================

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      // If user has an active selection, first ESC clears the selection
      if (!cropBox.classList.contains("hidden")) {
        clearSelection();
      } else {
        // Second ESC closes the overlay completely
        hideOverlay();
      }
    }
  });

  if (closeOverlayBtn) {
    closeOverlayBtn.addEventListener("click", () => {
      hideOverlay();
    });
  }

  if (hideBtn) {
    hideBtn.addEventListener("click", () => {
      hideOverlay();
    });
  }

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

  // =========================================================================
  // Tauri Event Listeners
  // =========================================================================
  if (window.__TAURI__?.event?.listen) {
    await window.__TAURI__.event.listen("screenshot-captured", (event) => {
      if (frozenImage && event.payload) {
        frozenImage.src = event.payload;
        clearSelection();
        setViewMode("overlay");
      }
    });

    await window.__TAURI__.event.listen("view-mode", (event) => {
      if (event.payload === "overlay") {
        setViewMode("overlay");
      } else {
        setViewMode("settings");
      }
    });
  }
});
