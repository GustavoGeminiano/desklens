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
  const toast = document.getElementById("toast");

  let currentDataUrl = null;
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
    };
    img.src = dataUrl;
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
