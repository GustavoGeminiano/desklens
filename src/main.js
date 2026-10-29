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
  const cropTextHighlights = document.getElementById("crop-text-highlights");

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
    if (y < 32) {
      cropDimensions.style.top = "6px";
      cropDimensions.style.left = "6px";
    } else {
      cropDimensions.style.top = "-28px";
      cropDimensions.style.left = "0px";
    }
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

    if (cropTextHighlights) {
      cropTextHighlights.innerHTML = "";
    }
    if (ocrActionBtn) {
      ocrActionBtn.innerHTML = "<span>📝</span> Copy Text";
    }
    if (searchActionBtn) {
      searchActionBtn.innerHTML = "<span>🔍</span> Search Image";
    }
  }

  // Render interactive word highlights directly over text in selection box (Sprint 4)
  function renderTextHighlights(ocrResult, cropW, cropH) {
    if (!cropTextHighlights || !ocrResult?.lines) return;
    cropTextHighlights.innerHTML = "";

    const tempImg = new Image();
    tempImg.onload = () => {
      const realW = tempImg.naturalWidth || cropW;
      const realH = tempImg.naturalHeight || cropH;
      const scaleX = currentRect.w / realW;
      const scaleY = currentRect.h / realH;

      for (const line of ocrResult.lines) {
        if (!line.words) continue;
        for (const word of line.words) {
          if (!word.text || !word.width || !word.height) continue;
          const pill = document.createElement("div");
          pill.className = "crop-word-highlight";
          pill.style.left = `${Math.round(word.x * scaleX)}px`;
          pill.style.top = `${Math.round(word.y * scaleY)}px`;
          pill.style.width = `${Math.round(word.width * scaleX)}px`;
          pill.style.height = `${Math.round(word.height * scaleY)}px`;
          pill.title = `Click to copy: "${word.text}"`;

          pill.addEventListener("mousedown", (e) => e.stopPropagation());
          pill.addEventListener("click", async (e) => {
            e.stopPropagation();
            try {
              await navigator.clipboard.writeText(word.text);
              pill.classList.add("copied");
              if (ocrActionBtn) {
                ocrActionBtn.innerHTML = `<span>📋</span> Copied: "${word.text}"`;
                setTimeout(() => {
                  ocrActionBtn.innerHTML = "<span>📝</span> Copy Text";
                }, 1600);
              }
              setTimeout(() => {
                pill.classList.remove("copied");
              }, 700);
            } catch (err) {
              console.error("Failed to copy word:", err);
            }
          });

          cropTextHighlights.appendChild(pill);
        }
      }
    };
    if (currentCropDataUrl) {
      tempImg.src = currentCropDataUrl;
    }
  }

  // Request backend Rust to crop the selected frame slice
  async function requestBackendCrop() {
    if (currentRect.w < 20 || currentRect.h < 20) return null;

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

          // Trigger OCR in background to highlight interactive words
          window.__TAURI__.core.invoke("extract_text_from_crop")
            .then((ocrRes) => {
              renderTextHighlights(ocrRes, currentRect.w, currentRect.h);
            })
            .catch(() => {});

          return croppedDataUrl;
        }
      } catch (err) {
        console.error("Failed to crop selection:", err);
      }
    }
    return null;
  }

  // Position floating action toolbar near the selection, clamped within screen bounds
  function positionActionToolbar() {
    cropActions.classList.remove("hidden");

    const margin = 12;
    const toolbarW = cropActions.offsetWidth || 330;
    const toolbarH = cropActions.offsetHeight || 46;

    // 1. Horizontal clamping so buttons never get cut off at left or right screen edges
    const idealScreenX = currentRect.x + (currentRect.w - toolbarW) / 2;
    const minScreenX = margin;
    const maxScreenX = Math.max(margin, window.innerWidth - toolbarW - margin);
    const clampedScreenX = Math.min(Math.max(idealScreenX, minScreenX), maxScreenX);
    cropActions.style.left = `${Math.round(clampedScreenX - currentRect.x)}px`;

    // 2. Vertical clamping (prefer below cropBox, flip above if near bottom, or inside if full-height)
    const spaceBelow = window.innerHeight - (currentRect.y + currentRect.h);
    const spaceAbove = currentRect.y;

    if (spaceBelow >= toolbarH + margin + 8) {
      cropActions.style.top = `${Math.round(currentRect.h + 10)}px`;
      cropActions.style.bottom = "auto";
    } else if (spaceAbove >= toolbarH + margin + 8) {
      cropActions.style.top = `${Math.round(-toolbarH - 10)}px`;
      cropActions.style.bottom = "auto";
    } else {
      cropActions.style.top = `${Math.max(8, Math.round(currentRect.h - toolbarH - 10))}px`;
      cropActions.style.bottom = "auto";
    }
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

  // Prevent pointer/mouse events on floating toolbar from bubbling to selectionLayer
  if (cropActions) {
    ["mousedown", "mouseup", "pointerdown", "click"].forEach((evtName) => {
      cropActions.addEventListener(evtName, (e) => {
        e.stopPropagation();
      });
    });
  }

  // Action Buttons
  if (clearActionBtn) {
    clearActionBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      clearSelection();
    });
  }

  if (searchActionBtn) {
    searchActionBtn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const originalHtml = searchActionBtn.innerHTML;
      searchActionBtn.innerHTML = "<span>⏳</span> Opening...";

      try {
        if (!currentCropDataUrl) {
          await requestBackendCrop();
        }

        if (window.__TAURI__?.core?.invoke) {
          await window.__TAURI__.core.invoke("open_visual_search", {
            cropDataUrl: currentCropDataUrl,
          });
          clearSelection();
          setViewMode("settings");
        }
      } catch (err) {
        console.error("Failed to open visual search panel:", err);
      } finally {
        searchActionBtn.innerHTML = originalHtml;
      }
    });
  }

  if (ocrActionBtn) {
    ocrActionBtn.addEventListener("click", async (e) => {
      e.stopPropagation();
      const originalHtml = ocrActionBtn.innerHTML;
      ocrActionBtn.innerHTML = "<span>⏳</span> Reading text...";

      try {
        if (!currentCropDataUrl) {
          await requestBackendCrop();
        }

        if (window.__TAURI__?.core?.invoke) {
          const extractedText = await window.__TAURI__.core.invoke("copy_text_from_crop");
          if (extractedText && extractedText.trim().length > 0) {
            ocrActionBtn.innerHTML = "<span>✅</span> Text Copied!";
            try {
              await navigator.clipboard.writeText(extractedText);
            } catch (_) {}
            setTimeout(() => {
              hideOverlay();
              ocrActionBtn.innerHTML = originalHtml;
            }, 600);
          } else {
            ocrActionBtn.innerHTML = "<span>⚠️</span> No text found";
            setTimeout(() => {
              ocrActionBtn.innerHTML = originalHtml;
            }, 1800);
          }
        }
      } catch (err) {
        console.error("Failed to copy OCR text:", err);
        ocrActionBtn.innerHTML = "<span>⚠️</span> OCR Error";
        setTimeout(() => {
          ocrActionBtn.innerHTML = originalHtml;
        }, 1800);
      }
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
