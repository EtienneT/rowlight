"use strict";

(() => {
  const tabs = [...document.querySelectorAll(".install-tabs a")];
  const panels = [...document.querySelectorAll(".install-panel")];
  if (!tabs.length) return;
  const tablist = document.querySelector(".install-tabs");
  tablist.setAttribute("role", "tablist");

  function activate(index, focus = false) {
    tabs.forEach((tab, position) => {
      tab.setAttribute("aria-selected", String(position === index));
      tab.tabIndex = position === index ? 0 : -1;
      panels[position].hidden = position !== index;
    });
    if (focus) tabs[index].focus();
  }
  function fromHash() {
    let fragment;
    try { fragment = decodeURIComponent(location.hash.slice(1)); } catch { return -1; }
    const target = document.getElementById(fragment);
    for (let ancestor = target?.parentElement; ancestor; ancestor = ancestor.parentElement) {
      if (ancestor.tagName === "DETAILS") ancestor.open = true;
    }
    return panels.findIndex((panel) => target && (target === panel || panel.contains(target)));
  }
  tabs.forEach((tab, index) => {
    tab.setAttribute("role", "tab");
    tab.setAttribute("aria-controls", panels[index].id);
    panels[index].setAttribute("role", "tabpanel");
    panels[index].setAttribute("aria-labelledby", tab.id);
    tab.addEventListener("click", (event) => {
      event.preventDefault();
      activate(index);
      history.replaceState(null, "", tab.getAttribute("href"));
    });
    tab.addEventListener("keydown", (event) => {
      let next = index;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      else if (event.key !== " ") return;
      event.preventDefault();
      activate(next, true);
      history.replaceState(null, "", tabs[next].getAttribute("href"));
    });
  });
  const linked = fromHash();
  activate(linked >= 0 ? linked : /Windows|Win32|Win64/.test(navigator.userAgent) ? 1 : 0);
  window.addEventListener("hashchange", () => {
    const index = fromHash();
    if (index >= 0) activate(index);
  });

  document.querySelectorAll(".guide-content pre").forEach((pre) => {
    const code = pre.querySelector("code");
    if (!code) return;
    const container = document.createElement("div");
    container.className = "install-command";
    const toolbar = document.createElement("div");
    toolbar.className = "install-command-toolbar";
    const label = document.createElement("span");
    label.textContent = code.classList.contains("language-powershell") ? "PowerShell" : "Terminal";
    const copy = document.createElement("button");
    copy.type = "button";
    copy.className = "quiet-button";
    copy.textContent = "Copy";
    copy.setAttribute("aria-label", "Copy " + label.textContent + " command");
    const status = document.createElement("span");
    status.className = "sr-only";
    status.setAttribute("role", "status");
    copy.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(code.textContent.trim());
        copy.textContent = "Copied";
        status.textContent = "Command copied.";
      } catch {
        copy.textContent = "Select command";
        status.textContent = "Clipboard unavailable. Select and copy the command.";
        const range = document.createRange();
        range.selectNodeContents(code);
        const selection = window.getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
      }
    });
    toolbar.append(label, copy, status);
    pre.replaceWith(container);
    container.append(toolbar, pre);
  });
})();
