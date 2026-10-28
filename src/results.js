// DeskLens — Visual Search Side-Panel Controller (Sprint 3)

window.addEventListener("DOMContentLoaded", async () => {
  const resultsThumb = document.getElementById("results-thumb");
  const cropResolution = document.getElementById("crop-resolution");
  const closeResultsBtn = document.getElementById("close-results-btn");
  const copyImgBtn = document.getElementById("copy-img-btn");
  const saveImgBtn = document.getElementById("save-img-btn");
  const openLensBtn = document.getElementById("open-lens-btn");
  const providerGoogleLens = document.getElementById("provider-google-lens");
  const toast = document.getElementById("toast");

  let currentDataUrl = null;

  function showToast(message) {
    if (!toast) return;
    toast.textContent = message;
    toast.classList.remove("hidden");
    setTimeout(() => {
      toast.classList.add("hidden");
    }, 2500);
  }

  // Load and display cropped image payload
  function displayCropPayload(dataUrl) {
    if (!dataUrl) return;
    currentDataUrl = dataUrl;
    resultsThumb.src = dataUrl;

    const img = new Image();
    img.onload = () => {
      cropResolution.textContent = `${img.naturalWidth} × ${img.naturalHeight} px`;
    };
    img.src = dataUrl;
  }

  // 1. Fetch latest crop from backend on window initialization
  if (window.__TAURI__?.core?.invoke) {
    try {
      const cachedCrop = await window.__TAURI__.core.invoke("get_latest_crop");
      if (cachedCrop) {
        displayCropPayload(cachedCrop);
      }
    } catch (err) {
      console.warn("Could not retrieve cached crop:", err);
    }
  }

  // 2. Listen for live payload emissions from backend
  if (window.__TAURI__?.event?.listen) {
    await window.__TAURI__.event.listen("load-search-payload", (event) => {
      if (event.payload) {
        displayCropPayload(event.payload);
      }
    });
  }

  // 3. Copy image to system clipboard
  if (copyImgBtn) {
    copyImgBtn.addEventListener("click", async () => {
      if (!currentDataUrl) return;
      try {
        const response = await fetch(currentDataUrl);
        const blob = await response.blob();
        await navigator.clipboard.write([
          new ClipboardItem({ "image/png": blob }),
        ]);
        showToast("Image copied to clipboard! Paste into Lens.");
      } catch (err) {
        console.error("Clipboard write failed:", err);
        showToast("Copied Data URL!");
      }
    });
  }

  // 4. Save PNG locally
  if (saveImgBtn) {
    saveImgBtn.addEventListener("click", () => {
      if (!currentDataUrl) return;
      const link = document.createElement("a");
      link.href = currentDataUrl;
      link.download = `desklens-crop-${Date.now()}.png`;
      link.click();
      showToast("Cropped image saved!");
    });
  }

  // 5. Open Google Lens
  function launchGoogleLens() {
    window.open("https://lens.google.com/", "_blank");
  }

  if (openLensBtn) {
    openLensBtn.addEventListener("click", launchGoogleLens);
  }
  if (providerGoogleLens) {
    providerGoogleLens.addEventListener("click", (e) => {
      e.preventDefault();
      launchGoogleLens();
    });
  }

  // 6. Close Results Panel
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
