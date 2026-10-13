"use strict";

(() => {
  const themes = {
    "tokyo-night": "Tokyo Night",
    catppuccin: "Catppuccin",
    gruvbox: "Gruvbox",
    "flexoki-light": "Flexoki Light",
  };

  // Keep the existing preference key across the branding change.
  function setTheme(theme) {
    if (!Object.hasOwn(themes, theme)) return;
    document.documentElement.dataset.theme = theme;
    document.querySelectorAll(".theme-button").forEach((button) => {
      button.setAttribute("aria-pressed", String(button.dataset.theme === theme));
    });
    const screenshot = document.getElementById("theme-image");
    if (screenshot) {
      screenshot.src = `assets/${theme}.png`;
      screenshot.alt = `Tablelamp running on Linux in ${themes[theme]}, showing a synthetic Parquet file.`;
      document.getElementById("theme-caption").textContent = themes[theme];
    }
    document.querySelector('meta[name="theme-color"]').content = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
    try { localStorage.setItem("rowlight-site-theme", theme); } catch { /* Storage is optional. */ }
  }

  document.querySelectorAll(".theme-button").forEach((button) => {
    button.addEventListener("click", () => setTheme(button.dataset.theme));
  });
  try { setTheme(localStorage.getItem("rowlight-site-theme")); } catch { /* Use the default theme. */ }
})();
