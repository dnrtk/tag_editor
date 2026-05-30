"use strict";

const state = {
  dir: null,
  files: [],          // image filenames in current directory
  current: null,      // absolute path of currently displayed image
  tags: [],
  hotkeys: {},        // {keyChar: tag}
  roots: [],          // pre-registered shared folders [{name, path}]
  restricted: false,  // true when the server limits access to the shared folders
  selectedTags: new Set(),
  searchSelectedTags: new Set(),
  searchBase: null,    // base folder of the last successful recursive search
  searchResults: [],   // [{path, rel}] from the last search
  slideshow: {
    timer: null,
    list: [],
    index: 0,
  },
};

const PATH_SEP = (() => {
  // Heuristic: if the user types a Windows-style absolute path, use '\\' as separator.
  return navigator.platform.startsWith("Win") ? "\\" : "/";
})();

function $(id) { return document.getElementById(id); }
function setStatus(msg) { $("status").textContent = msg; }

function joinPath(dir, name) {
  if (dir.endsWith("/") || dir.endsWith("\\")) return dir + name;
  return dir + PATH_SEP + name;
}

// Global busy indicator. Every fetch goes through api(), so reference-counting
// here lights the top-bar spinner whenever any request is in flight — making it
// clear the app is working rather than frozen, even on slow recursive scans.
let busyCount = 0;
function setBusy(delta) {
  busyCount = Math.max(0, busyCount + delta);
  const sp = $("busy-spinner");
  if (sp) sp.hidden = busyCount === 0;
}

async function api(url, opts = {}) {
  setBusy(1);
  try {
    const r = await fetch(url, opts);
    if (!r.ok) {
      let msg = `${r.status} ${r.statusText}`;
      try { const body = await r.json(); if (body.error) msg = body.error; } catch (_) {}
      throw new Error(msg);
    }
    return r;
  } finally {
    setBusy(-1);
  }
}

async function apiJson(url, opts = {}) {
  const r = await api(url, opts);
  return r.json();
}

// -----------------------------------------------------------------------------
// Shared folders (pre-registered open targets)
// -----------------------------------------------------------------------------

async function loadRoots() {
  try {
    const data = await apiJson("/api/roots");
    state.roots = data.roots || [];
    state.restricted = !!data.restricted;
    renderRoots();
  } catch (e) {
    setStatus("Shared folders: " + e.message);
  }
}

/// Normalizes a path for tolerant comparison: unify separators, drop a trailing
/// slash, lowercase (host paths here are Windows, case-insensitive).
function normPath(p) {
  return String(p).replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

function isRoot(p) {
  const n = normPath(p);
  return state.roots.some(r => normPath(r.path) === n);
}

function renderRoots() {
  const section = $("roots-section");
  const ul = $("roots-list");
  ul.innerHTML = "";
  if (state.roots.length === 0) {
    section.classList.add("hidden");
    return;
  }
  section.classList.remove("hidden");
  for (const r of state.roots) {
    const li = document.createElement("li");
    li.textContent = "🔖 " + r.name;
    li.title = r.path;
    li.onclick = () => openDir(r.path);
    ul.appendChild(li);
  }
}

// -----------------------------------------------------------------------------
// Directory and file list
// -----------------------------------------------------------------------------

async function openDir(dir) {
  try {
    const url = `/api/tree?path=${encodeURIComponent(dir)}`;
    const data = await apiJson(url);
    state.dir = data.path;
    state.files = data.files;
    $("path-input").value = data.path;

    const parentLink = $("parent-link");
    // Hide the up-link at a shared root under restriction — its parent is outside
    // the allowlist and would 403 anyway.
    if (data.parent && !(state.restricted && isRoot(data.path))) {
      parentLink.innerHTML = `<a id="go-parent">⬆ ${data.parent}</a>`;
      $("go-parent").onclick = () => openDir(data.parent);
    } else {
      parentLink.textContent = "";
    }

    $("dir-list").innerHTML = "";
    for (const name of data.dirs) {
      const li = document.createElement("li");
      li.textContent = "📁 " + name;
      li.title = name;
      li.onclick = () => openDir(joinPath(data.path, name));
      $("dir-list").appendChild(li);
    }

    renderFileList();
    setStatus(`${state.files.length} image(s) in folder`);
  } catch (e) {
    setStatus("Error: " + e.message);
  }
}

function renderFileList() {
  const ul = $("file-list");
  ul.innerHTML = "";
  for (const name of state.files) {
    const li = document.createElement("li");
    li.textContent = "🖼 " + name;
    li.title = name;
    const fullPath = joinPath(state.dir, name);
    if (state.current === fullPath) li.classList.add("current");
    li.onclick = () => openImage(fullPath);
    ul.appendChild(li);
  }
}

// -----------------------------------------------------------------------------
// Image and tags
// -----------------------------------------------------------------------------

async function openImage(path) {
  state.current = path;
  const v = $("viewer");
  const frame = $("image-frame");
  // Show the overlay spinner until the image decodes, so a slow load during a
  // slideshow reads as "loading next frame" rather than a frozen view.
  if (frame) frame.classList.add("loading");
  v.onload = () => frame && frame.classList.remove("loading");
  v.onerror = () => frame && frame.classList.remove("loading");
  v.src = `/api/image?path=${encodeURIComponent(path)}&t=${Date.now()}`;
  $("image-name").textContent = path.split(/[\\/]/).pop();
  renderFileList();
  await loadTags(path);
}

/// Returns to the no-image-selected state. Mirrors the desktop Esc behavior.
function closeImage() {
  if (!state.current) return;
  state.current = null;
  state.tags = [];
  const v = $("viewer");
  // Remove src so the broken-image icon doesn't show; CSS hides empty <img>.
  v.removeAttribute("src");
  const frame = $("image-frame");
  if (frame) frame.classList.remove("loading");
  $("image-name").textContent = "";
  renderTags();
  renderFileList();
  setStatus("Closed");
}

async function loadTags(path) {
  try {
    const data = await apiJson(`/api/tags?path=${encodeURIComponent(path)}`);
    state.tags = data.tags || [];
    renderTags();
  } catch (e) {
    setStatus("Tags: " + e.message);
  }
}

function renderTags() {
  const ul = $("tag-list");
  ul.innerHTML = "";
  for (const tag of state.tags) {
    const li = document.createElement("li");
    li.innerHTML = `<span>• ${escapeHtml(tag)}</span>`;
    const btn = document.createElement("button");
    btn.textContent = "✕";
    btn.onclick = () => removeTag(tag);
    li.appendChild(btn);
    ul.appendChild(li);
  }
}

async function saveTags() {
  if (!state.current) return;
  try {
    await api(`/api/tags?path=${encodeURIComponent(state.current)}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tags: state.tags }),
    });
    setStatus("Tags saved");
  } catch (e) {
    setStatus("Save error: " + e.message);
  }
}

function addTag(name) {
  const trimmed = name.trim();
  if (!trimmed) return;
  if (!state.tags.includes(trimmed)) state.tags.push(trimmed);
  renderTags();
}

function removeTag(name) {
  state.tags = state.tags.filter(t => t !== name);
  renderTags();
}

async function toggleTag(name) {
  const trimmed = name.trim();
  if (!trimmed) return;
  if (state.tags.includes(trimmed)) {
    state.tags = state.tags.filter(t => t !== trimmed);
  } else {
    state.tags.push(trimmed);
  }
  renderTags();
  await saveTags();
}

// -----------------------------------------------------------------------------
// Hotkeys (server-defined tag shortcuts)
// -----------------------------------------------------------------------------

async function loadHotkeys() {
  try {
    const data = await apiJson("/api/hotkeys");
    state.hotkeys = data.hotkeys || {};
    renderHotkeys();
    renderHotkeyChips();
    renderSearchChips();
  } catch (e) {
    setStatus("Hotkeys: " + e.message);
  }
}

function renderHotkeys() {
  const ul = $("hotkey-list");
  ul.innerHTML = "";
  const keys = Object.keys(state.hotkeys).sort();
  for (const key of keys) {
    const tag = state.hotkeys[key];
    const li = document.createElement("li");
    li.innerHTML = `<span class="key">${escapeHtml(key)}</span><span>${escapeHtml(tag)}</span>`;
    const btn = document.createElement("button");
    btn.textContent = "Toggle";
    btn.className = "apply";
    btn.onclick = () => toggleTag(tag);
    li.appendChild(btn);
    ul.appendChild(li);
  }
}

function renderHotkeyChips() {
  const wrap = $("hotkey-chips");
  wrap.innerHTML = "";
  const keys = Object.keys(state.hotkeys).sort();
  for (const key of keys) {
    const tag = state.hotkeys[key];
    const chip = document.createElement("span");
    chip.className = "chip" + (state.selectedTags.has(tag) ? " selected" : "");
    chip.textContent = `[${key}] ${tag}`;
    chip.onclick = () => {
      if (state.selectedTags.has(tag)) state.selectedTags.delete(tag);
      else state.selectedTags.add(tag);
      renderHotkeyChips();
      refreshFilter();
    };
    wrap.appendChild(chip);
  }
}

// -----------------------------------------------------------------------------
// Slideshow filter
// -----------------------------------------------------------------------------

let filterDebounceTimer = null;
function refreshFilter() {
  if (filterDebounceTimer) clearTimeout(filterDebounceTimer);
  filterDebounceTimer = setTimeout(doRefreshFilter, 200);
}

async function doRefreshFilter() {
  const tags = Array.from(state.selectedTags).join(",");
  const q = $("filter-q").value;
  const base = $("slideshow-base").value.trim();
  try {
    if (base) {
      // Recursive mode: search the base folder and all its subfolders. Each match
      // carries an absolute path plus a base-relative label for display.
      const url = `/api/search?path=${encodeURIComponent(base)}&tags=${encodeURIComponent(tags)}&q=${encodeURIComponent(q)}`;
      const data = await apiJson(url);
      renderFilterList((data.matches || []).map(m => ({ path: m.path, label: m.rel })));
    } else {
      // Current-folder mode: top level only.
      if (!state.dir) { renderFilterList([]); return; }
      const url = `/api/filter?path=${encodeURIComponent(state.dir)}&tags=${encodeURIComponent(tags)}&q=${encodeURIComponent(q)}`;
      const data = await apiJson(url);
      renderFilterList((data.matches || []).map(name => ({ path: joinPath(state.dir, name), label: name })));
    }
  } catch (e) {
    setStatus("Filter: " + e.message);
  }
}

/// Renders the slideshow candidate list from `[{path, label}]` items. `path` is
/// the absolute path used to open/thumbnail the image; `label` is what the user
/// sees (a bare filename in current-folder mode, a relative path in recursive mode).
function renderFilterList(items) {
  $("filter-count").textContent = `Matched: ${items.length}`;
  const ul = $("filter-list");
  ul.innerHTML = "";
  const view = $("view-mode").value;
  // Toggle the layout class so the CSS grid switches on for thumbnail mode.
  ul.classList.toggle("thumbs-mode", view === "thumbs");

  for (const item of items) {
    const li = document.createElement("li");
    li.dataset.path = item.path;
    // The browser tooltip surfaces the label when truncated (names mode) or
    // entirely hidden (thumbs mode).
    li.title = item.label;
    if (view === "thumbs") {
      li.className = "thumb";
      const img = document.createElement("img");
      img.src = `/api/thumb?path=${encodeURIComponent(item.path)}&size=96`;
      img.loading = "lazy";
      img.alt = item.label;
      li.appendChild(img);
    } else {
      li.className = "name";
      li.textContent = item.label;
    }
    li.onclick = () => openImage(item.path);
    ul.appendChild(li);
  }
}

function startSlideshow() {
  const ul = $("filter-list");
  // Read the path directly from each item — the rendered text varies between
  // names and thumbnail modes, but dataset.path is always set.
  const list = Array.from(ul.children).map(li => li.dataset.path).filter(Boolean);
  if (list.length === 0) {
    setStatus("No images match the current filter");
    return;
  }
  stopSlideshow();
  state.slideshow.list = list;
  state.slideshow.index = 0;
  openImage(list[0]);

  const interval = parseFloat($("interval").value) || 3.0;
  const loop = $("loop").checked;
  state.slideshow.timer = setInterval(() => {
    state.slideshow.index += 1;
    if (state.slideshow.index >= state.slideshow.list.length) {
      if (loop) state.slideshow.index = 0;
      else { stopSlideshow(); setStatus("Slideshow completed"); return; }
    }
    openImage(state.slideshow.list[state.slideshow.index]);
  }, interval * 1000);
  setStatus("Slideshow started");
}

function stopSlideshow() {
  if (state.slideshow.timer) {
    clearInterval(state.slideshow.timer);
    state.slideshow.timer = null;
  }
}

// -----------------------------------------------------------------------------
// Recursive search + bulk export
// -----------------------------------------------------------------------------

function openSearchPopup() {
  // Default the base folder to the directory currently being browsed.
  if (!$("search-base").value && state.dir) $("search-base").value = state.dir;
  $("search-popup").classList.remove("hidden");
}

function closeSearchPopup() {
  $("search-popup").classList.add("hidden");
}

function renderSearchChips() {
  const wrap = $("search-chips");
  wrap.innerHTML = "";
  const keys = Object.keys(state.hotkeys).sort();
  for (const key of keys) {
    const tag = state.hotkeys[key];
    const chip = document.createElement("span");
    chip.className = "chip" + (state.searchSelectedTags.has(tag) ? " selected" : "");
    chip.textContent = `[${key}] ${tag}`;
    chip.onclick = () => {
      if (state.searchSelectedTags.has(tag)) state.searchSelectedTags.delete(tag);
      else state.searchSelectedTags.add(tag);
      renderSearchChips();
      refreshSearch();
    };
    wrap.appendChild(chip);
  }
}

let searchDebounceTimer = null;
function refreshSearch() {
  if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
  searchDebounceTimer = setTimeout(doSearch, 250);
}

async function doSearch() {
  const base = $("search-base").value.trim();
  if (!base) { setStatus("Enter a base folder to search"); return; }
  const tags = Array.from(state.searchSelectedTags).join(",");
  const q = $("search-q").value;
  const url = `/api/search?path=${encodeURIComponent(base)}&tags=${encodeURIComponent(tags)}&q=${encodeURIComponent(q)}`;
  setStatus("Searching…");
  try {
    const data = await apiJson(url);
    state.searchBase = data.base;
    state.searchResults = data.matches || [];
    renderSearchList();
    setStatus(`Search: ${state.searchResults.length} match(es)`);
  } catch (e) {
    setStatus("Search: " + e.message);
  }
}

function renderSearchList() {
  $("search-count").textContent = `Matched: ${state.searchResults.length}`;
  const ul = $("search-list");
  ul.innerHTML = "";
  const view = $("search-view").value;
  ul.classList.toggle("thumbs-mode", view === "thumbs");

  for (const item of state.searchResults) {
    const li = document.createElement("li");
    li.dataset.path = item.path;
    // Show the path relative to the search base so the subfolder is visible.
    li.title = item.rel;
    if (view === "thumbs") {
      li.className = "thumb";
      const img = document.createElement("img");
      img.src = `/api/thumb?path=${encodeURIComponent(item.path)}&size=96`;
      img.loading = "lazy";
      img.alt = item.rel;
      li.appendChild(img);
    } else {
      li.className = "name";
      li.textContent = item.rel;
    }
    li.onclick = () => openImage(item.path);
    ul.appendChild(li);
  }
}

async function doExport() {
  if (state.searchResults.length === 0) { setStatus("Nothing to export"); return; }
  const dest = $("search-dest").value.trim();
  if (!dest) { setStatus("Enter an export destination folder"); return; }
  if (!state.searchBase) { setStatus("Run a search before exporting"); return; }
  setStatus("Exporting…");
  try {
    const data = await apiJson("/api/export", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        base: state.searchBase,
        dest,
        files: state.searchResults.map(r => r.path),
      }),
    });
    const errs = data.errors || [];
    setStatus(errs.length
      ? `Exported ${data.copied}, ${errs.length} failed (${errs[0]})`
      : `Exported ${data.copied} file(s) to ${dest}`);
  } catch (e) {
    setStatus("Export: " + e.message);
  }
}

// -----------------------------------------------------------------------------
// Navigation
// -----------------------------------------------------------------------------

function navigate(direction) {
  if (!state.current || state.files.length === 0) return;
  const name = state.current.split(/[\\/]/).pop();
  const idx = state.files.indexOf(name);
  if (idx < 0) return;
  const len = state.files.length;
  const nextIdx = (idx + direction + len) % len;
  openImage(joinPath(state.dir, state.files[nextIdx]));
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// -----------------------------------------------------------------------------
// Event wiring
// -----------------------------------------------------------------------------

window.addEventListener("DOMContentLoaded", async () => {
  $("path-go").onclick = () => openDir($("path-input").value);
  $("path-input").addEventListener("keydown", e => {
    if (e.key === "Enter") openDir($("path-input").value);
  });
  $("prev-btn").onclick = () => navigate(-1);
  $("next-btn").onclick = () => navigate(1);
  $("save-btn").onclick = saveTags;
  $("add-btn").onclick = () => {
    addTag($("new-tag").value);
    $("new-tag").value = "";
  };
  $("new-tag").addEventListener("keydown", e => {
    if (e.key === "Enter") {
      addTag(e.target.value);
      e.target.value = "";
    }
  });
  $("filter-q").addEventListener("input", refreshFilter);
  $("filter-btn").onclick = doRefreshFilter;
  $("view-mode").addEventListener("change", doRefreshFilter);
  $("slideshow-base").addEventListener("input", refreshFilter);
  $("slideshow-base").addEventListener("keydown", e => {
    if (e.key === "Enter") doRefreshFilter();
  });
  $("slideshow-base-clear").onclick = () => {
    $("slideshow-base").value = "";
    doRefreshFilter();
  };
  $("start-btn").onclick = startSlideshow;
  $("stop-btn").onclick = () => { stopSlideshow(); setStatus("Slideshow stopped"); };

  $("slideshow-open").onclick = openSlideshowPopup;
  $("slideshow-close").onclick = closeSlideshowPopup;
  initFloatingDrag($("slideshow-popup"), $("slideshow-popup-header"), "tag_editor.slideshow_pos");

  $("search-open").onclick = openSearchPopup;
  $("search-close").onclick = closeSearchPopup;
  $("search-btn").onclick = doSearch;
  $("search-q").addEventListener("input", refreshSearch);
  $("search-base").addEventListener("keydown", e => {
    if (e.key === "Enter") doSearch();
  });
  $("search-view").addEventListener("change", renderSearchList);
  $("search-export").onclick = doExport;
  initFloatingDrag($("search-popup"), $("search-popup-header"), "tag_editor.search_pos");

  document.addEventListener("keydown", e => {
    // Ctrl+S works regardless of focus, mirroring the desktop behavior.
    if (e.ctrlKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      saveTags();
      return;
    }
    // Single-key shortcuts must not fire while typing in form controls.
    const tag = e.target.tagName;
    if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
    if (e.key === "ArrowLeft") navigate(-1);
    else if (e.key === "ArrowRight") navigate(1);
    else if (e.key === "Escape") closeImage();
    else if (state.hotkeys[e.key]) toggleTag(state.hotkeys[e.key]);
  });

  initSplitter();
  await loadHotkeys();
  await loadRoots();
  // Default landing dir comes from the server when no path is supplied: the first
  // shared folder when access is restricted, otherwise the home directory.
  await openDir("");
});

/// Shows the slideshow setup popup and refreshes the filter so the user sees the
/// current matches as soon as the panel appears.
function openSlideshowPopup() {
  $("slideshow-popup").classList.remove("hidden");
  doRefreshFilter();
}

function closeSlideshowPopup() {
  $("slideshow-popup").classList.add("hidden");
}

/// Makes a fixed-position element draggable by its header. Position is persisted
/// to localStorage so the panel reopens where the user last placed it.
function initFloatingDrag(panel, handle, storageKey) {
  if (!panel || !handle) return;

  // Restore saved position. Clamp into viewport in case the window shrank.
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) || "null");
    if (saved && Number.isFinite(saved.left) && Number.isFinite(saved.top)) {
      panel.style.left = clamp(saved.left, 0, window.innerWidth - 100) + "px";
      panel.style.top = clamp(saved.top, 0, window.innerHeight - 60) + "px";
    }
  } catch (_) { /* malformed localStorage; ignore */ }

  let dragging = false;
  let offsetX = 0;
  let offsetY = 0;

  handle.addEventListener("mousedown", e => {
    // Don't start a drag if the user clicked the close button or another control.
    if (e.target.closest("button")) return;
    dragging = true;
    const rect = panel.getBoundingClientRect();
    offsetX = e.clientX - rect.left;
    offsetY = e.clientY - rect.top;
    e.preventDefault();
  });

  document.addEventListener("mousemove", e => {
    if (!dragging) return;
    const left = clamp(e.clientX - offsetX, 0, window.innerWidth - 100);
    const top = clamp(e.clientY - offsetY, 0, window.innerHeight - 40);
    panel.style.left = left + "px";
    panel.style.top = top + "px";
  });

  document.addEventListener("mouseup", () => {
    if (!dragging) return;
    dragging = false;
    const rect = panel.getBoundingClientRect();
    localStorage.setItem(storageKey, JSON.stringify({ left: rect.left, top: rect.top }));
  });
}

function clamp(n, lo, hi) {
  return Math.max(lo, Math.min(hi, n));
}

/// Drag handle between left sidebar and center pane. The chosen width is persisted
/// so the layout survives reload.
function initSplitter() {
  const sp = $("splitter-left");
  const layout = document.querySelector(".layout");
  if (!sp || !layout) return;

  const STORAGE_KEY = "tag_editor.left_w";
  const stored = parseInt(localStorage.getItem(STORAGE_KEY) || "", 10);
  if (!Number.isNaN(stored) && stored >= 120 && stored <= 800) {
    layout.style.setProperty("--left-w", stored + "px");
  }

  let dragging = false;
  let startX = 0;
  let startW = 0;

  sp.addEventListener("mousedown", e => {
    dragging = true;
    sp.classList.add("dragging");
    startX = e.clientX;
    startW = sp.previousElementSibling.getBoundingClientRect().width;
    document.body.style.cursor = "ew-resize";
    e.preventDefault();
  });

  document.addEventListener("mousemove", e => {
    if (!dragging) return;
    const newW = Math.max(120, Math.min(800, startW + (e.clientX - startX)));
    layout.style.setProperty("--left-w", newW + "px");
  });

  document.addEventListener("mouseup", () => {
    if (!dragging) return;
    dragging = false;
    sp.classList.remove("dragging");
    document.body.style.cursor = "";
    const current = sp.previousElementSibling.getBoundingClientRect().width;
    localStorage.setItem(STORAGE_KEY, String(Math.round(current)));
  });
}
