"use strict";

(() => {
  // A bounded, synthetic illustration. This page never reads visitors' files.
  const regions = ["North", "East", "South", "West", "East", "Central"];
  const rows = Array.from({ length: 32 }, (_, index) => ({
    id: index + 1,
    amount: index % 13 === 0 ? null : Number((85 + 35 * Math.sin(index * 0.17)).toFixed(2)),
    active: index % 11 === 0 ? null : index % 5 !== 0,
    region: regions[index % regions.length],
    note: index % 7 === 0 ? "Review requested" : null,
    created_at: `2024-10-${String(index % 28 + 1).padStart(2, "0")}`,
  }));
  const columns = [
    { key: "id", type: "u64", kind: "numeric" },
    { key: "amount", type: "f64", kind: "numeric" },
    { key: "active", type: "bool", kind: "category" },
    { key: "region", type: "str", kind: "category" },
    { key: "note", type: "str", kind: "category" },
    { key: "created_at", type: "date", kind: "category" },
  ];
  const themes = {
    "tokyo-night": "Tokyo Night",
    catppuccin: "Catppuccin",
    gruvbox: "Gruvbox",
    "flexoki-light": "Flexoki Light",
  };
  const pageSize = 8;
  let page = 0;
  let sortKey = null;
  let sortDirection = 0;
  let plotsVisible = true;
  const head = document.getElementById("demo-head");
  const body = document.getElementById("demo-body");
  const inspector = document.getElementById("inspector");
  const content = document.getElementById("inspector-content");

  function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  function display(value) {
    return value === null ? "null" : String(value);
  }

  function currentRows() {
    return rows.slice(page * pageSize, (page + 1) * pageSize);
  }

  function distribution(column) {
    const values = currentRows().map((row) => row[column.key]).filter((value) => value !== null);
    if (column.kind === "numeric") {
      const min = Math.min(...values);
      const max = Math.max(...values);
      const bins = Array(8).fill(0);
      values.forEach((value) => {
        const bin = max === min ? 0 : Math.min(7, Math.floor((value - min) / (max - min) * 8));
        bins[bin] += 1;
      });
      return { counts: bins, label: `${min} – ${max}`, values, min, max };
    }
    const counts = new Map();
    values.forEach((value) => counts.set(display(value), (counts.get(display(value)) || 0) + 1));
    const entries = [...counts.entries()];
    return { counts: entries.map((entry) => entry[1]), label: `${entries.length} distinct · ${pageSize - values.length} nulls`, entries, values };
  }

  function bars(counts, className) {
    const chart = element("div", className);
    chart.setAttribute("aria-hidden", "true");
    const peak = Math.max(1, ...counts);
    counts.forEach((count) => {
      const bar = element("span");
      bar.style.height = `${count / peak * 100}%`;
      chart.append(bar);
    });
    return chart;
  }

  function openInspector(title, kind, metadata) {
    document.getElementById("inspector-title").textContent = title;
    document.getElementById("inspector-kind").textContent = kind;
    document.getElementById("inspector-meta").textContent = metadata;
    content.replaceChildren();
  }

  function inspectCell(row, column) {
    openInspector(column.key, "Cell inspector", `Source row ${row.id} · ${column.type}`);
    content.append(element("div", row[column.key] === null ? "null-value" : "", display(row[column.key])));
    inspector.showModal();
  }

  function inspectColumn(column) {
    const summary = distribution(column);
    openInspector(column.key, "Column distribution", `${column.type} · ${pageSize} source rows on page ${page + 1}`);
    content.append(bars(summary.counts, "distribution-bars"));
    const details = element("div", "distribution-values");
    if (column.kind === "numeric") {
      details.append(element("p", "", `Range: ${summary.label}`));
      details.append(element("p", "", `Mean: ${(summary.values.reduce((sum, value) => sum + value, 0) / summary.values.length).toFixed(2)}`));
    } else {
      summary.entries.forEach(([value, count]) => details.append(element("p", "", `${value}: ${count}`)));
    }
    details.append(element("p", "", `Nulls: ${pageSize - summary.values.length} / ${pageSize}`));
    content.append(details);
    inspector.showModal();
  }

  function render() {
    const headerRow = element("tr");
    const gutter = element("th");
    gutter.scope = "col";
    gutter.append(element("span", "gutter-heading", "#"));
    headerRow.append(gutter);
    columns.forEach((column) => {
      const header = element("th");
      header.scope = "col";
      header.setAttribute("aria-sort", sortKey === column.key && sortDirection ? (sortDirection === 1 ? "ascending" : "descending") : "none");
      const sort = element("button", "sort-button");
      sort.type = "button";
      sort.setAttribute("aria-label", `Sort ${column.key}`);
      const arrow = sortKey === column.key && sortDirection ? (sortDirection === 1 ? "↑" : "↓") : "↕";
      sort.append(element("span", "", column.key), element("span", "sort-arrow", arrow));
      sort.addEventListener("click", () => {
        sortDirection = sortKey === column.key ? (sortDirection + 1) % 3 : 1;
        sortKey = column.key;
        render();
        head.querySelector(`[aria-label="Sort ${column.key}"]`).focus({ preventScroll: true });
      });
      header.append(sort, element("span", "column-type", column.type));
      if (plotsVisible) {
        const summary = distribution(column);
        const plot = element("button", "plot-button");
        plot.type = "button";
        plot.setAttribute("aria-label", `Explore ${column.key} distribution`);
        plot.append(bars(summary.counts, "mini-plot"), element("span", "plot-label", summary.label));
        plot.addEventListener("click", () => inspectColumn(column));
        header.append(plot);
      }
      headerRow.append(header);
    });
    head.replaceChildren(headerRow);
    const visible = currentRows();
    if (sortDirection) {
      visible.sort((left, right) => {
        const a = left[sortKey];
        const b = right[sortKey];
        if (a === null) return b === null ? 0 : 1;
        if (b === null) return -1;
        return (a < b ? -1 : a > b ? 1 : 0) * (sortDirection === 1 ? 1 : -1);
      });
    }
    body.replaceChildren(...visible.map((row) => {
      const tr = element("tr");
      tr.append(element("td", "", row.id));
      columns.forEach((column) => {
        const td = element("td");
        const cell = element("button", `cell-button${row[column.key] === null ? " null-value" : ""}`, display(row[column.key]));
        cell.type = "button";
        cell.setAttribute("aria-label", `Inspect ${column.key}, source row ${row.id}: ${display(row[column.key])}`);
        cell.addEventListener("click", () => inspectCell(row, column));
        td.append(cell);
        tr.append(td);
      });
      return tr;
    }));
    document.getElementById("row-status").textContent = `Rows ${page * pageSize + 1}–${(page + 1) * pageSize} of ${rows.length} · ${columns.length} columns`;
    document.getElementById("page-status").textContent = `${page + 1} / ${rows.length / pageSize}`;
    document.getElementById("previous-page").disabled = page === 0;
    document.getElementById("next-page").disabled = (page + 1) * pageSize >= rows.length;
  }

  function changePage(direction) {
    page = Math.max(0, Math.min(rows.length / pageSize - 1, page + direction));
    render();
  }
  document.getElementById("previous-page").addEventListener("click", () => changePage(-1));
  document.getElementById("next-page").addEventListener("click", () => changePage(1));
  document.getElementById("plot-toggle").addEventListener("click", (event) => {
    plotsVisible = !plotsVisible;
    event.currentTarget.setAttribute("aria-pressed", String(plotsVisible));
    event.currentTarget.textContent = plotsVisible ? "Hide plots" : "Show plots";
    render();
  });
  document.getElementById("close-inspector").addEventListener("click", () => inspector.close());
  inspector.addEventListener("click", (event) => {
    const bounds = inspector.getBoundingClientRect();
    if (event.target === inspector && (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom)) inspector.close();
  });

  function setTheme(theme) {
    if (!Object.hasOwn(themes, theme)) return;
    document.documentElement.dataset.theme = theme;
    document.querySelectorAll(".theme-button").forEach((button) => button.setAttribute("aria-pressed", String(button.dataset.theme === theme)));
    const screenshot = document.getElementById("theme-image");
    screenshot.src = `assets/${theme}.png`;
    screenshot.alt = `Rowlight running on Linux in ${themes[theme]}, showing a synthetic Parquet file.`;
    document.getElementById("theme-caption").textContent = themes[theme];
    document.querySelector('meta[name="theme-color"]').content = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
    try { localStorage.setItem("rowlight-site-theme", theme); } catch { /* Storage is optional. */ }
  }
  document.querySelectorAll(".theme-button").forEach((button) => button.addEventListener("click", () => setTheme(button.dataset.theme)));
  try { setTheme(localStorage.getItem("rowlight-site-theme")); } catch { /* Use the default theme. */ }
  render();
  document.getElementById("demo-fallback").hidden = true;
  document.getElementById("interactive-demo").hidden = false;
  document.getElementById("plot-toggle").hidden = false;
  document.getElementById("demo-help").textContent = "Sort a column. Open a chart. Inspect a cell. Illustrative web demo · synthetic data.";
})();
