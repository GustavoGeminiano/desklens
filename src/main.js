// DeskLens — Vanilla JS Frontend Controller (Tauri v2)
window.addEventListener("DOMContentLoaded", () => {
  const hideBtn = document.getElementById("hide-btn");

  if (hideBtn) {
    hideBtn.addEventListener("click", async () => {
      if (window.__TAURI__ && window.__TAURI__.window) {
        const currentWindow = window.__TAURI__.window.getCurrentWindow();
        await currentWindow.hide();
      }
    });
  }
});
