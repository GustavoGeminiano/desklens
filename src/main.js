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
  const translateActionBtn = document.getElementById("translate-action-btn");
  const ocrActionBtn = document.getElementById("ocr-action-btn");
  const sourceLangSelect = document.getElementById("source-lang-select");
  const targetLangSelect = document.getElementById("target-lang-select");
  const langSelectorGroup = document.getElementById("lang-selector-group");

  // State
  let isSelecting = false;
  let isResizing = false;
  let activeHandle = null;
  let startX = 0;
  let startY = 0;
  let currentRect = { x: 0, y: 0, w: 0, h: 0 };
  let currentCropDataUrl = null;
  let activeTranslatedText = null; // Full translated text when a translation is shown

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
    if (langSelectorGroup) {
      langSelectorGroup.classList.add("hidden");
    }
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
    if (translateActionBtn) {
      translateActionBtn.innerHTML = "<span>🌐</span> Translate";
    }
    activeTranslatedText = null;
    clearTranslationOverlays();
  }

  // Copies text to the OS clipboard through the Rust backend (reliable on Wayland/X11),
  // falling back to the browser Clipboard API.
  async function copyTextToClipboard(text) {
    if (!text) return false;
    if (window.__TAURI__?.core?.invoke) {
      try {
        await window.__TAURI__.core.invoke("copy_text_to_clipboard", { text });
        return true;
      } catch (err) {
        console.warn("Backend clipboard copy failed, trying navigator.clipboard:", err);
      }
    }
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (_) {
      return false;
    }
  }
  window.copyTextToClipboard = copyTextToClipboard;

  // Joins a slice of pill items: words on the same line with spaces, different lines with newlines
  function joinPillItems(items) {
    let out = "";
    let lastLine = null;
    for (const it of items) {
      if (lastLine === null) {
        out = it.text;
      } else if (it.lineIndex === lastLine) {
        out += " " + it.text;
      } else {
        out += "\n" + it.text;
      }
      lastLine = it.lineIndex;
    }
    return out.trim();
  }

  function flashOcrButton(html) {
    if (!ocrActionBtn) return;
    ocrActionBtn.innerHTML = html;
    setTimeout(() => {
      ocrActionBtn.innerHTML = activeTranslatedText
        ? "<span>📝</span> Copy Translation"
        : "<span>📝</span> Copy Text";
    }, 1800);
  }

  // Builds interactive, copyable pills inside the crop box.
  // items: [{ text, x, y, width, height, lineIndex }] in crop-box CSS pixels.
  // Click = copy item • Drag across items = copy phrase • Double-click = copy whole line.
  function renderInteractivePills(items, { translated = false } = {}) {
    if (!cropTextHighlights) return;
    cropTextHighlights.innerHTML = "";

    const flatItems = [];
    items.forEach((item, idx) => {
      if (!item || !item.text || !item.width || !item.height) return;
      const pill = document.createElement("div");
      pill.className = translated ? "crop-word-highlight translated" : "crop-word-highlight";
      pill.dataset.index = String(flatItems.length);
      pill.dataset.line = String(item.lineIndex ?? idx);
      pill.style.left = `${Math.round(item.x)}px`;
      pill.style.top = `${Math.round(item.y)}px`;
      pill.style.width = `${Math.round(item.width)}px`;
      pill.style.height = `${Math.round(item.height)}px`;

      if (translated) {
        pill.textContent = item.text;
        const fontH = Math.max(12, item.height);
        let fSize = Math.max(10, Math.min(Math.round(fontH * 0.72), 22));
        if (item.text.length > 0) {
          const estimatedWidth = item.text.length * (fSize * 0.55);
          if (estimatedWidth > item.width * 1.2 && item.height < 35) {
            const factor = (item.width * 1.1) / estimatedWidth;
            fSize = Math.max(9, Math.round(fSize * Math.max(0.65, factor)));
          }
        }
        pill.style.fontSize = `${fSize}px`;
      }

      pill.title = translated
        ? `Tradução: "${item.text}" • Clique para copiar • Arraste para copiar várias linhas`
        : `Drag to select phrase • Double-click for full line • Click for word: "${item.text}"`;

      const pillObj = { index: flatItems.length, lineIndex: item.lineIndex ?? idx, text: item.text, pill };
      flatItems.push(pillObj);

      // 1. Click / drag across pills to copy a word or phrase
      pill.addEventListener("mousedown", (e) => {
        e.stopPropagation();
        e.preventDefault();
        const startIdx = pillObj.index;
        let currentEndIdx = startIdx;

        flatItems.forEach((w) => w.pill.classList.remove("selected"));
        pill.classList.add("selected");

        const onMove = (moveEvt) => {
          const targetEl = document.elementFromPoint(moveEvt.clientX, moveEvt.clientY);
          if (targetEl && targetEl.classList.contains("crop-word-highlight")) {
            const hoverIdx = parseInt(targetEl.dataset.index, 10);
            if (!isNaN(hoverIdx) && hoverIdx !== currentEndIdx) {
              currentEndIdx = hoverIdx;
              const minI = Math.min(startIdx, currentEndIdx);
              const maxI = Math.max(startIdx, currentEndIdx);
              flatItems.forEach((w) => {
                w.pill.classList.toggle("selected", w.index >= minI && w.index <= maxI);
              });
            }
          }
        };

        const onUp = async () => {
          window.removeEventListener("mousemove", onMove);
          window.removeEventListener("mouseup", onUp);

          const minI = Math.min(startIdx, currentEndIdx);
          const maxI = Math.max(startIdx, currentEndIdx);
          const selectedSlice = flatItems.slice(minI, maxI + 1);
          const phrase = joinPillItems(selectedSlice);
          if (!phrase) return;

          const ok = await copyTextToClipboard(phrase);
          selectedSlice.forEach((w) => {
            w.pill.classList.remove("selected");
            if (ok) {
              w.pill.classList.add("copied");
              setTimeout(() => w.pill.classList.remove("copied"), 800);
            }
          });
          const snippet = phrase.length > 25 ? phrase.slice(0, 25) + "..." : phrase;
          flashOcrButton(ok ? `<span>📋</span> Copied: "${snippet.replace(/\n/g, " ")}"` : "<span>⚠️</span> Copy failed");
        };

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
      });

      // 2. Double click copies the whole line/sentence
      pill.addEventListener("dblclick", async (e) => {
        e.stopPropagation();
        const lineItems = flatItems.filter((w) => w.lineIndex === pillObj.lineIndex);
        const lineSentence = joinPillItems(lineItems);
        if (!lineSentence) return;
        const ok = await copyTextToClipboard(lineSentence);
        if (ok) {
          lineItems.forEach((w) => {
            w.pill.classList.add("copied");
            setTimeout(() => w.pill.classList.remove("copied"), 900);
          });
          flashOcrButton("<span>📋</span> Copied Line!");
        }
      });

      cropTextHighlights.appendChild(pill);
    });
  }

  // Render interactive word highlights directly over text in selection box (Sprint 4)
  function renderTextHighlights(ocrResult, cropW, cropH) {
    if (!cropTextHighlights || !ocrResult?.lines) return;
    cropTextHighlights.innerHTML = "";

    const tempImg = new Image();
    tempImg.onload = () => {
      // A translation may have been applied meanwhile; never overwrite translated pills
      if (activeTranslatedText) return;
      const realW = tempImg.naturalWidth || cropW;
      const realH = tempImg.naturalHeight || cropH;
      const scaleX = currentRect.w / realW;
      const scaleY = currentRect.h / realH;

      const items = [];
      ocrResult.lines.forEach((line, lineIdx) => {
        (line.words || []).forEach((word) => {
          if (!word.text || !word.width || !word.height) return;
          items.push({
            text: word.text,
            x: word.x * scaleX,
            y: word.y * scaleY,
            width: word.width * scaleX,
            height: word.height * scaleY,
            lineIndex: lineIdx,
          });
        });
      });
      renderInteractivePills(items, { translated: false });
    };
    if (currentCropDataUrl) {
      tempImg.src = currentCropDataUrl;
    }
  }

  // =========================================================================
  // Google Lens Translation Overlays (Requirement R3)
  // =========================================================================

  // Clear all active translation overlays from #capture-container
  function clearTranslationOverlays() {
    const container = document.getElementById("capture-container");
    if (container) {
      const overlays = container.querySelectorAll(".lens-translation-box");
      overlays.forEach((el) => el.remove());
    }
  }

  // Dynamically render translation overlay boxes covering original text
  function renderTranslationOverlays(boxes) {
    clearTranslationOverlays();
    if (!boxes || !Array.isArray(boxes)) return;

    const container = document.getElementById("capture-container") || document.body;

    boxes.forEach((box) => {
      if (!box || typeof box.x !== "number" || typeof box.y !== "number") return;

      const numW = Number(box.width);
      const numH = Number(box.height);
      const boxW = Number.isFinite(numW) ? numW : 0;
      const boxH = Number.isFinite(numH) ? numH : 0;

      const overlay = document.createElement("div");
      overlay.className = "lens-translation-box";
      overlay.style.left = `${Math.round(box.x)}px`;
      overlay.style.top = `${Math.round(box.y)}px`;
      overlay.style.width = `${Math.round(boxW)}px`;
      overlay.style.height = `${Math.round(boxH)}px`;

      const displayText = box.translated_text || box.text || "";
      overlay.textContent = displayText;

      // Responsive font size calculation
      const fontBoxH = Math.max(12, Math.abs(boxH));
      const fontBoxW = Math.max(20, Math.abs(boxW));
      let fontSize = Math.max(10, Math.min(Math.round(fontBoxH * 0.72), 22));

      if (displayText.length > 0) {
        const estimatedWidth = displayText.length * (fontSize * 0.55);
        if (estimatedWidth > boxW * 1.2 && boxH < 35) {
          const factor = (boxW * 1.1) / estimatedWidth;
          fontSize = Math.max(9, Math.round(fontSize * Math.max(0.65, factor)));
        }
      }
      overlay.style.fontSize = `${fontSize}px`;

      // Tooltip and click-to-copy
      overlay.title = `Original: "${box.text || ""}"\nClick to copy translation`;
      overlay.addEventListener("click", async (e) => {
        e.stopPropagation();
        if (displayText) {
          const copied = await copyTextToClipboard(displayText);
          if (copied) {
            overlay.classList.add("copied");
            if (typeof setTimeout !== "undefined") {
              setTimeout(() => overlay.classList.remove("copied"), 800);
            }
          }
        }
      });

      container.appendChild(overlay);
    });
  }

  window.renderTranslationOverlays = renderTranslationOverlays;
  window.clearTranslationOverlays = clearTranslationOverlays;

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
    const toolbarW = cropActions.offsetWidth || 520;
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
    clearTranslationOverlays();
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

        // Hide the fullscreen overlay immediately on click so the user never has to press ESC
        // and the always-on-top overlay never blocks the browser window from taking focus.
        await hideOverlay();

        if (window.__TAURI__?.core?.invoke) {
          await window.__TAURI__.core.invoke("search_google_lens");
        }
      } catch (err) {
        console.error("Failed to open Google Lens:", err);
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

        // When a translation is displayed, copy the translated text instead of the original
        if (activeTranslatedText) {
          const ok = await copyTextToClipboard(activeTranslatedText);
          ocrActionBtn.innerHTML = ok ? "<span>✅</span> Translation Copied!" : "<span>⚠️</span> Copy failed";
          setTimeout(() => {
            ocrActionBtn.innerHTML = "<span>📝</span> Copy Translation";
          }, 1800);
          return;
        }

        if (window.__TAURI__?.core?.invoke) {
          const extractedText = await window.__TAURI__.core.invoke("copy_text_from_crop");
          if (extractedText && extractedText.trim().length > 0) {
            ocrActionBtn.innerHTML = "<span>✅</span> Text Copied!";
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
  // Language Selection Setup & Persistence (Requirement R2)
  // =========================================================================
  const STORAGE_KEY_SOURCE_LANG = "desklens_source_lang";
  const STORAGE_KEY_TARGET_LANG = "desklens_target_lang";

  function getStoredSourceLang() {
    try {
      return localStorage.getItem(STORAGE_KEY_SOURCE_LANG) || "auto";
    } catch (_) {
      return "auto";
    }
  }

  function getStoredTargetLang() {
    try {
      return localStorage.getItem(STORAGE_KEY_TARGET_LANG) || "pt";
    } catch (_) {
      return "pt";
    }
  }

  function updateTranslateButtonTitle() {
    if (!translateActionBtn) return;
    const targetText = targetLangSelect?.options?.[targetLangSelect.selectedIndex]?.text || targetLangSelect?.value || "Portuguese";
    translateActionBtn.title = `Translate on screen (${targetText})`;
  }

  if (sourceLangSelect) {
    sourceLangSelect.value = getStoredSourceLang();
    sourceLangSelect.addEventListener("change", () => {
      try {
        localStorage.setItem(STORAGE_KEY_SOURCE_LANG, sourceLangSelect.value);
      } catch (_) {}
      if (activeTranslatedText) {
        triggerTranslation();
      }
    });
  }

  if (targetLangSelect) {
    targetLangSelect.value = getStoredTargetLang();
    updateTranslateButtonTitle();
    targetLangSelect.addEventListener("change", () => {
      try {
        localStorage.setItem(STORAGE_KEY_TARGET_LANG, targetLangSelect.value);
      } catch (_) {}
      updateTranslateButtonTitle();
      if (activeTranslatedText) {
        triggerTranslation();
      }
    });
  }

  // Hide language selectors by default until the user clicks Translate
  if (langSelectorGroup) {
    langSelectorGroup.classList.add("hidden");
  }

  // =========================================================================
  // Translate Action Button Handler (Requirement R3)
  // =========================================================================
  async function triggerTranslation() {
    if (!translateActionBtn) return;
    const originalHtml = translateActionBtn.innerHTML;
    translateActionBtn.innerHTML = "<span>⏳</span> Translating...";

    if (langSelectorGroup) {
      langSelectorGroup.classList.remove("hidden");
      repositionCropActions();
    }

    try {
      if (!currentCropDataUrl) {
        await requestBackendCrop();
      }

      if (window.__TAURI__?.core?.invoke) {
        // 1. Get OCR bounding boxes from current crop
        const ocrResult = await window.__TAURI__.core.invoke("extract_text_from_crop");
        const flatBoxes = [];

        if (ocrResult?.boxes && ocrResult.boxes.length > 0) {
          for (const b of ocrResult.boxes) {
            if (b.text && b.width && b.height) {
              flatBoxes.push({
                text: b.text,
                x: b.x,
                y: b.y,
                width: b.width,
                height: b.height,
              });
            }
          }
        } else if (ocrResult?.lines) {
          for (const line of ocrResult.lines) {
            if (line.words) {
              for (const word of line.words) {
                if (word.text && word.width && word.height) {
                  flatBoxes.push({
                    text: word.text,
                    x: word.x,
                    y: word.y,
                    width: word.width,
                    height: word.height,
                  });
                }
              }
            }
          }
        }

        if (flatBoxes.length === 0) {
          translateActionBtn.innerHTML = "<span>⚠️</span> No text found";
          setTimeout(() => {
            translateActionBtn.innerHTML = originalHtml;
          }, 1800);
          return;
        }

        const selectedTargetLang = targetLangSelect ? targetLangSelect.value : "pt";
        const selectedSourceLang = sourceLangSelect ? sourceLangSelect.value : "auto";

        // 2. Invoke translation command
        const translatedBoxes = await window.__TAURI__.core.invoke("translate_on_screen", {
          boxes: flatBoxes,
          targetLang: selectedTargetLang,
          sourceLang: selectedSourceLang,
        });

        // Snapshot current selection rect to prevent stale rendering if user modifies crop
        const rectSnapshot = { ...currentRect };

        // 3. Map crop pixel coordinates to container CSS coordinates
        const tempImg = new Image();
        tempImg.onload = () => {
          if (rectSnapshot.w <= 0 || rectSnapshot.h <= 0) {
            translateActionBtn.innerHTML = originalHtml;
            return;
          }
          const realW = tempImg.naturalWidth || rectSnapshot.w;
          const realH = tempImg.naturalHeight || rectSnapshot.h;
          const scaleX = rectSnapshot.w / realW;
          const scaleY = rectSnapshot.h / realH;

          const mappedBoxes = (translatedBoxes || []).map((b) => ({
            ...b,
            x: rectSnapshot.x + Math.round(b.x * scaleX),
            y: rectSnapshot.y + Math.round(b.y * scaleY),
            width: Math.round(b.width * scaleX),
            height: Math.round(b.height * scaleY),
          }));

          // Resolve vertical collisions / overlaps between adjacent lines in document OCR
          for (let i = 0; i < mappedBoxes.length; i++) {
            const b1 = mappedBoxes[i];
            if (b1.width <= 0 || b1.height <= 0) continue;
            for (let j = 0; j < mappedBoxes.length; j++) {
              if (i === j) continue;
              const b2 = mappedBoxes[j];
              if (b2.width <= 0 || b2.height <= 0) continue;
              // If b2 starts inside b1's vertical span and b2 is at or below b1
              if (b2.y >= b1.y && b2.y < b1.y + b1.height) {
                const hOverlap = Math.max(0, Math.min(b1.x + b1.width, b2.x + b2.width) - Math.max(b1.x, b2.x));
                const minW = Math.min(b1.width, b2.width);
                if (minW > 0 && hOverlap / minW > 0.3) {
                  b2.y = b1.y + b1.height + 2;
                }
              }
            }
          }

          renderTranslationOverlays(mappedBoxes);

          // Clear interactive word highlight layer so it doesn't double-render or overlap
          if (cropTextHighlights) {
            cropTextHighlights.innerHTML = "";
          }

          // Store full translated text for the Copy action
          activeTranslatedText = (translatedBoxes || [])
            .map((b) => (b.translated_text || b.text || "").trim())
            .filter(Boolean)
            .join("\n");

          if (ocrActionBtn) {
            ocrActionBtn.innerHTML = "<span>📝</span> Copy Translation";
          }

          translateActionBtn.innerHTML = "<span>✅</span> Translated!";
          setTimeout(() => {
            translateActionBtn.innerHTML = originalHtml;
          }, 2000);
        };
        tempImg.onerror = (err) => {
          console.error("Failed to decode cropped image for translation overlays:", err);
          translateActionBtn.innerHTML = originalHtml;
        };
        tempImg.src = currentCropDataUrl;
      }
    } catch (err) {
      console.error("Failed to translate on screen:", err);
      translateActionBtn.innerHTML = "<span>⚠️</span> Error";
      setTimeout(() => {
        translateActionBtn.innerHTML = originalHtml;
      }, 1800);
    }
  }

  if (translateActionBtn) {
    translateActionBtn.addEventListener("click", async (e) => {
      e.stopPropagation();
      await triggerTranslation();
    });
  }

  // =========================================================================
  // Keyboard & Overlay Exit Controls
  // =========================================================================

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hideOverlay();
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
        frozenImage.onload = () => {
          clearSelection();
          setViewMode("overlay");
        };
        frozenImage.src = event.payload;
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
