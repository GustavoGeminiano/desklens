// DeskLens — Visual Search Side-Panel Controller (Sprint 3)

window.addEventListener("DOMContentLoaded", async () => {
  const resultsThumb = document.getElementById("results-thumb");
  const cropResolution = document.getElementById("crop-resolution");
  const cropStatus = document.getElementById("crop-status");
  const closeResultsBtn = document.getElementById("close-results-btn");
  const copyImgBtn = document.getElementById("copy-img-btn");
  const saveImgBtn = document.getElementById("save-img-btn");
  const openLensBtn = document.getElementById("open-lens-btn");
  const providerGoogleLens = document.getElementById("provider-google-lens");
  const providerGoogleImages = document.getElementById("provider-google-images");
  const ocrTextBox = document.getElementById("ocr-text-box");
  const ocrWordCount = document.getElementById("ocr-word-count");
  const copyOcrBtn = document.getElementById("copy-ocr-btn");
  const ocrHighlightOverlay = document.getElementById("ocr-highlight-overlay");
  const toast = document.getElementById("toast");

  let currentDataUrl = null;
  let latestExtractedText = "";
  let toastTimeout = null;

  function showToast(message, durationMs = 3000) {
    if (!toast) return;
    if (toastTimeout) {
      clearTimeout(toastTimeout);
    }
    toast.textContent = message;
    toast.classList.remove("hidden");
    toastTimeout = setTimeout(() => {
      toast.classList.add("hidden");
    }, durationMs);
  }

  // Load and display cropped image payload
  function displayCropPayload(dataUrl) {
    if (!dataUrl) return;
    currentDataUrl = dataUrl;
    resultsThumb.src = dataUrl;

    const img = new Image();
    img.onload = () => {
      cropResolution.textContent = `${img.naturalWidth} × ${img.naturalHeight} px`;
      if (cropStatus) {
        cropStatus.textContent = "Crop Ready";
      }
      if (copyOcrBtn) {
        copyOcrBtn.textContent = "📋 Copy";
      }
      triggerOcrDetection(img.naturalWidth, img.naturalHeight);
    };
    img.src = dataUrl;
  }

  // Render clickable bounding-box word pills over the preview image (Google Lens style)
  function renderOcrPills(result, naturalW, naturalH) {
    if (!ocrHighlightOverlay) return;
    ocrHighlightOverlay.innerHTML = "";
    if (!result?.lines || !resultsThumb) return;

    const displayedW = resultsThumb.clientWidth;
    const displayedH = resultsThumb.clientHeight;
    if (!displayedW || !displayedH || !naturalW || !naturalH) return;

    const scaleX = displayedW / naturalW;
    const scaleY = displayedH / naturalH;

    const flatWords = [];
    let globalWordIdx = 0;

    for (let lineIdx = 0; lineIdx < result.lines.length; lineIdx++) {
      const line = result.lines[lineIdx];
      if (!line.words) continue;

      for (const word of line.words) {
        if (!word.text || !word.width || !word.height) continue;
        const pill = document.createElement("div");
        pill.className = "ocr-word-pill";
        pill.dataset.index = globalWordIdx;
        pill.dataset.line = lineIdx;
        pill.style.left = `${Math.round(word.x * scaleX)}px`;
        pill.style.top = `${Math.round(word.y * scaleY)}px`;
        pill.style.width = `${Math.round(word.width * scaleX)}px`;
        pill.style.height = `${Math.round(word.height * scaleY)}px`;
        pill.title = `Drag to select phrase • Double-click for line • Click for word: "${word.text}"`;

        const wordObj = {
          index: globalWordIdx,
          lineIndex: lineIdx,
          text: word.text,
          pill,
        };
        flatWords.push(wordObj);

        // 1. Drag across words to select a phrase
        pill.addEventListener("mousedown", (e) => {
          e.stopPropagation();
          const startIdx = parseInt(pill.dataset.index, 10);
          let currentEndIdx = startIdx;

          flatWords.forEach((w) => w.pill.classList.remove("selected"));
          pill.classList.add("selected");

          const onMove = (moveEvt) => {
            const targetEl = document.elementFromPoint(moveEvt.clientX, moveEvt.clientY);
            if (targetEl && targetEl.classList.contains("ocr-word-pill")) {
              const hoverIdx = parseInt(targetEl.dataset.index, 10);
              if (!isNaN(hoverIdx) && hoverIdx !== currentEndIdx) {
                currentEndIdx = hoverIdx;
                const minI = Math.min(startIdx, currentEndIdx);
                const maxI = Math.max(startIdx, currentEndIdx);

                flatWords.forEach((w) => {
                  if (w.index >= minI && w.index <= maxI) {
                    w.pill.classList.add("selected");
                  } else {
                    w.pill.classList.remove("selected");
                  }
                });
              }
            }
          };

          const onUp = async () => {
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);

            const minI = Math.min(startIdx, currentEndIdx);
            const maxI = Math.max(startIdx, currentEndIdx);
            const selectedSlice = flatWords.slice(minI, maxI + 1);
            const phrase = selectedSlice.map((w) => w.text).join(" ").trim();

            if (phrase) {
              try {
                await navigator.clipboard.writeText(phrase);
                selectedSlice.forEach((w) => {
                  w.pill.classList.remove("selected");
                  w.pill.classList.add("copied");
                  setTimeout(() => w.pill.classList.remove("copied"), 800);
                });

                if (selectedSlice.length > 1) {
                  const snippet = phrase.length > 30 ? phrase.slice(0, 30) + "..." : phrase;
                  showToast(`📋 Copied phrase: "${snippet}"`);
                } else {
                  showToast(`📋 Copied: "${phrase}"`);
                }
              } catch (err) {
                console.error("Failed to copy selected text:", err);
              }
            }
          };

          window.addEventListener("mousemove", onMove);
          window.addEventListener("mouseup", onUp);
        });

        // 2. Double-click copies the whole line/sentence
        pill.addEventListener("dblclick", async (e) => {
          e.stopPropagation();
          const thisLineIdx = parseInt(pill.dataset.line, 10);
          const lineWords = flatWords.filter((w) => w.lineIndex === thisLineIdx);
          const lineSentence = lineWords.map((w) => w.text).join(" ").trim();

          if (lineSentence) {
            try {
              await navigator.clipboard.writeText(lineSentence);
              lineWords.forEach((w) => {
                w.pill.classList.add("copied");
                setTimeout(() => w.pill.classList.remove("copied"), 900);
              });
              showToast(`📋 Copied full line: "${lineSentence.slice(0, 35)}..."`);
            } catch (err) {
              console.error("Failed to copy line:", err);
            }
          }
        });

        ocrHighlightOverlay.appendChild(pill);
        globalWordIdx++;
      }
    }
  }

  // Trigger native OS OCR text recognition
  async function triggerOcrDetection(naturalW, naturalH) {
    if (!window.__TAURI__?.core?.invoke) return;
    if (ocrTextBox) {
      ocrTextBox.value = "Detecting text with Windows OCR...";
    }
    if (ocrHighlightOverlay) {
      ocrHighlightOverlay.innerHTML = "";
    }
    try {
      const result = await window.__TAURI__.core.invoke("extract_text_from_crop");
      if (result && result.full_text && result.full_text.trim().length > 0) {
        latestExtractedText = result.full_text.trim();
        if (ocrTextBox) ocrTextBox.value = latestExtractedText;
        if (ocrWordCount) ocrWordCount.textContent = `${result.word_count} word${result.word_count === 1 ? "" : "s"}`;
        renderOcrPills(result, naturalW, naturalH);
      } else {
        latestExtractedText = "";
        if (ocrTextBox) ocrTextBox.value = "No text detected in this image.";
        if (ocrWordCount) ocrWordCount.textContent = "0 words";
      }
    } catch (err) {
      console.warn("OCR recognition error:", err);
      if (ocrTextBox) ocrTextBox.value = "Text recognition error.";
    }
  }

  // Fetch latest crop from Rust backend state
  async function refreshCropFromBackend() {
    if (window.__TAURI__?.core?.invoke) {
      try {
        const cachedCrop = await window.__TAURI__.core.invoke("get_latest_crop");
        if (cachedCrop && cachedCrop !== currentDataUrl) {
          displayCropPayload(cachedCrop);
        }
      } catch (err) {
        console.warn("Could not retrieve cached crop:", err);
      }
    }
  }

  // Copy recognized OCR text to clipboard
  if (copyOcrBtn) {
    copyOcrBtn.addEventListener("click", async () => {
      if (!latestExtractedText) {
        showToast("⚠️ No text to copy");
        return;
      }
      try {
        await navigator.clipboard.writeText(latestExtractedText);
        showToast("✅ Text copied to clipboard!");
        copyOcrBtn.textContent = "✅ Copied!";
        setTimeout(() => {
          copyOcrBtn.textContent = "📋 Copy";
        }, 1500);
      } catch (err) {
        console.error("Clipboard copy failed:", err);
      }
    });
  }

  // 1. Fetch latest crop on initialization, focus, and visibility changes
  await refreshCropFromBackend();
  window.addEventListener("focus", refreshCropFromBackend);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) {
      refreshCropFromBackend();
    }
  });

  // 2. Listen for live payload emissions from backend
  if (window.__TAURI__?.event?.listen) {
    await window.__TAURI__.event.listen("load-search-payload", (event) => {
      if (event.payload) {
        displayCropPayload(event.payload);
      }
    });
  }

  // 3. Copy image to system clipboard via native Rust command
  if (copyImgBtn) {
    copyImgBtn.addEventListener("click", async () => {
      await refreshCropFromBackend();
      try {
        if (window.__TAURI__?.core?.invoke) {
          await window.__TAURI__.core.invoke("copy_crop_to_clipboard");
          showToast("✅ Image copied to system clipboard!");
          return;
        }
      } catch (err) {
        console.warn("Native clipboard copy fallback:", err);
      }

      // Fallback to WebView Clipboard API
      if (currentDataUrl) {
        try {
          const response = await fetch(currentDataUrl);
          const blob = await response.blob();
          await navigator.clipboard.write([
            new ClipboardItem({ "image/png": blob }),
          ]);
          showToast("✅ Image copied to clipboard!");
        } catch (err) {
          console.error("Clipboard write failed:", err);
          showToast("⚠️ Could not copy image to clipboard");
        }
      }
    });
  }

  // 4. Save PNG locally via native Rust command (saves to Pictures/DeskLens & reveals in Explorer)
  if (saveImgBtn) {
    saveImgBtn.addEventListener("click", async () => {
      await refreshCropFromBackend();
      if (window.__TAURI__?.core?.invoke) {
        try {
          await window.__TAURI__.core.invoke("save_crop_to_disk");
          showToast("💾 Saved to Pictures/DeskLens!");
          return;
        } catch (err) {
          console.error("Failed to save PNG via backend:", err);
          showToast("⚠️ Failed to save PNG");
        }
      }
    });
  }

  // 5. Upload crop directly to Google Lens and open results in integrated WebView
  async function launchGoogleLens() {
    await refreshCropFromBackend();
    if (!window.__TAURI__?.core?.invoke) return;

    showToast("⏳ Uploading crop to Google Lens...", 6000);
    if (cropStatus) {
      cropStatus.textContent = "Uploading to Lens...";
    }

    try {
      await window.__TAURI__.core.invoke("search_google_lens");
      showToast("🔍 Google Lens results opened!");
      if (cropStatus) {
        cropStatus.textContent = "Search Complete";
      }
    } catch (err) {
      console.error("Google Lens upload failed:", err);
      showToast("⚠️ Failed to open Google Lens");
      if (cropStatus) {
        cropStatus.textContent = "Upload Error";
      }
    }
  }

  if (openLensBtn) {
    openLensBtn.addEventListener("click", launchGoogleLens);
  }
  if (providerGoogleLens) {
    providerGoogleLens.addEventListener("click", launchGoogleLens);
  }

  // 6. Open Google Images in default browser (auto-copies image to clipboard)
  if (providerGoogleImages) {
    providerGoogleImages.addEventListener("click", async () => {
      await refreshCropFromBackend();
      if (window.__TAURI__?.core?.invoke) {
        try {
          showToast("📋 Image copied! Press Ctrl+V in Google Images", 4500);
          await window.__TAURI__.core.invoke("open_search_provider", {
            provider: "google-images",
          });
        } catch (err) {
          console.error("Failed to open Google Images:", err);
        }
      }
    });
  }

  // 7. Close Results Panel
  async function closeResults() {
    if (window.__TAURI__?.core?.invoke) {
      await window.__TAURI__.core.invoke("close_results_window");
    } else if (window.__TAURI__?.window) {
      await window.__TAURI__.window.getCurrentWindow().hide();
    }
  }

  if (closeResultsBtn) {
    closeResultsBtn.addEventListener("click", closeResults);
  }

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      closeResults();
    }
  });
});
