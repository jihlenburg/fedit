const { invoke } = window.__TAURI__.core;
const { open, save: saveDialog, ask } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

window.addEventListener("DOMContentLoaded", () => {
  const greetInput = document.querySelector("#greet-input");
  const greetMsg = document.querySelector("#greet-msg");
  document.querySelector("#greet-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    greetMsg.textContent = await invoke("greet", { name: greetInput.value });
  });

  const echoInput = document.querySelector("#echo-input");
  const echoMsg = document.querySelector("#echo-msg");
  document.querySelector("#echo-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    echoMsg.textContent = await invoke("echo", { msg: echoInput.value });
  });

  const pathText = document.querySelector("#path-text");
  const dirtyDot = document.querySelector("#dirty-dot");
  const editor = document.querySelector("#editor");
  const gutter = document.querySelector("#gutter");
  const tabBar = document.querySelector("#tab-bar");

  // --------------------------------------------------------------------------
  // Tab state (ch23)
  //
  // Rust owns the tab list (ids, paths, dirty flags). JS owns the editor
  // CONTENTS for each tab — the textarea can only show one buffer at a time,
  // so we cache the others in `buffers` keyed by tab id. Switching a tab =
  // stash editor.value into the outgoing tab, load editor.value from the
  // incoming tab.
  // --------------------------------------------------------------------------
  const buffers = new Map(); // id -> string
  let activeId = null;

  // Render line numbers into the gutter, one per line in the textarea.
  function renderGutter() {
    const lineCount = editor.value.length === 0 ? 1 : editor.value.split("\n").length;
    const nums = new Array(lineCount);
    for (let i = 0; i < lineCount; i++) nums[i] = i + 1;
    gutter.textContent = nums.join("\n");
  }
  function syncGutterScroll() {
    gutter.scrollTop = editor.scrollTop;
  }

  function shortName(path) {
    if (!path) return "untitled";
    const bits = path.split(/[\\/]/);
    return bits[bits.length - 1] || path;
  }

  function renderTabs(tabs) {
    tabBar.replaceChildren();
    for (const t of tabs) {
      const chip = document.createElement("div");
      chip.className = "tab" + (t.id === activeId ? " active" : "");
      chip.setAttribute("role", "tab");
      chip.setAttribute("aria-selected", t.id === activeId ? "true" : "false");
      chip.dataset.id = t.id;

      if (t.dirty) {
        const dot = document.createElement("span");
        dot.className = "tab-dirty";
        dot.textContent = "●";
        dot.setAttribute("aria-label", "unsaved");
        chip.appendChild(dot);
      }

      const name = document.createElement("span");
      name.className = "name";
      name.textContent = shortName(t.path);
      name.title = t.path ?? "untitled";
      chip.appendChild(name);

      const close = document.createElement("button");
      close.className = "close";
      close.type = "button";
      close.textContent = "✕";
      close.setAttribute("aria-label", `close ${shortName(t.path)}`);
      close.addEventListener("click", (e) => {
        e.stopPropagation();
        closeTab(t.id, t.dirty);
      });
      chip.appendChild(close);

      chip.addEventListener("click", () => {
        if (t.id !== activeId) switchToTab(t.id);
      });

      tabBar.appendChild(chip);
    }

    const plus = document.createElement("button");
    plus.className = "tab tab-new";
    plus.type = "button";
    plus.textContent = "+";
    plus.setAttribute("aria-label", "new tab");
    plus.addEventListener("click", () => createNewTab());
    tabBar.appendChild(plus);
  }

  async function refreshTabs() {
    const tabs = await invoke("list_tabs");
    renderTabs(tabs);
    return tabs;
  }

  function setPathLine(tab) {
    pathText.textContent = tab?.path ?? (tab ? "untitled" : "");
    dirtyDot.hidden = !(tab && tab.dirty);
  }

  // Stash the current textarea contents into the outgoing tab's buffer.
  function stashActive() {
    if (activeId !== null) buffers.set(activeId, editor.value);
  }

  // Load the given tab into the textarea.
  async function loadTab(tab) {
    activeId = tab.id;
    editor.value = buffers.get(tab.id) ?? "";
    renderGutter();
    setPathLine(tab);
  }

  async function createNewTab() {
    stashActive();
    const tab = await invoke("new_tab");
    buffers.set(tab.id, "");
    await loadTab(tab);
    await refreshTabs();
    editor.focus();
  }

  async function switchToTab(id) {
    stashActive();
    await invoke("switch_tab", { id });
    const tab = await invoke("active_tab");
    if (tab) await loadTab(tab);
    await refreshTabs();
  }

  async function closeTab(id, dirty) {
    if (dirty) {
      // Tauri 2 WebViews don't expose window.confirm — use the dialog plugin's
      // `ask`, which renders a native OS dialog and returns a boolean promise.
      const confirmed = await ask("This tab has unsaved changes. Close anyway?", {
        title: "fedit",
        kind: "warning",
      });
      if (!confirmed) return;
    }
    stashActive();
    const tabs = await invoke("close_tab", { id });
    buffers.delete(id);
    if (tabs.length === 0) {
      // Always keep at least one tab so the editor has somewhere to live.
      const fresh = await invoke("new_tab");
      buffers.set(fresh.id, "");
      await loadTab(fresh);
      await refreshTabs();
      return;
    }
    // Backend already adjusted `active`; read it back.
    const active = await invoke("active_tab");
    if (active) await loadTab(active);
    await refreshTabs();
  }

  async function doSave(newPath) {
    if (activeId === null) return null;
    try {
      const saved = await invoke("save", {
        id: activeId,
        newPath: newPath ?? null,
        contents: editor.value,
      });
      pathText.textContent = `${saved} — saved`;
      dirtyDot.hidden = true;
      await refreshTabs();
      return saved;
    } catch (err) {
      if (err && err.kind === "NoPath") {
        const chosen = await saveDialog({});
        if (chosen === null) return null;
        return doSave(chosen);
      }
      pathText.textContent = `error: ${err?.message ?? err}`;
      return null;
    }
  }

  const recentList = document.querySelector("#recent-list");
  async function refreshRecent() {
    const paths = await invoke("recent_files");
    recentList.replaceChildren(
      ...paths.map((p) => {
        const li = document.createElement("li");
        li.textContent = p;
        li.addEventListener("click", () => openPath(p));
        return li;
      }),
    );
  }

  async function openPath(path) {
    stashActive();
    try {
      const opened = await invoke("open_tab", { path });
      // If we switched to an existing tab that has an in-memory buffer, prefer
      // the cached version (user may have unsaved edits). Otherwise seed the
      // buffer with on-disk contents.
      if (!opened.already_open || !buffers.has(opened.tab.id)) {
        buffers.set(opened.tab.id, opened.contents);
      }
      await loadTab(opened.tab);
      await refreshTabs();
      await refreshRecent();
    } catch (err) {
      pathText.textContent = `${path} — error: ${err?.message ?? err}`;
    }
  }

  // Boot: make sure there's always at least one tab on first paint.
  async function boot() {
    const tabs = await invoke("list_tabs");
    if (tabs.length === 0) {
      const tab = await invoke("new_tab");
      buffers.set(tab.id, "");
      await loadTab(tab);
    } else {
      const active = await invoke("active_tab");
      for (const t of tabs) if (!buffers.has(t.id)) buffers.set(t.id, "");
      if (active) await loadTab(active);
    }
    await refreshTabs();
    await refreshRecent();
  }
  boot();

  document.querySelector("#open-btn").addEventListener("click", async () => {
    const path = await open({ multiple: false, directory: false });
    if (path === null) return;
    await openPath(path);
  });

  document.querySelector("#save-btn").addEventListener("click", async () => {
    await doSave(null);
    await refreshRecent();
  });

  document.querySelector("#save-as-btn").addEventListener("click", async () => {
    const chosen = await saveDialog({});
    if (chosen === null) return;
    await doSave(chosen);
    await refreshRecent();
  });

  editor.addEventListener("input", async () => {
    if (activeId === null) return;
    buffers.set(activeId, editor.value);
    // Mark the active tab dirty in Rust, then refresh the tab strip so the
    // dirty dot shows up. This is one IPC per keystroke transition — cheap
    // because we guard with a local "was already dirty?" check.
    if (dirtyDot.hidden) {
      dirtyDot.hidden = false;
      await invoke("set_tab_dirty", { id: activeId, dirty: true });
      await refreshTabs();
    }
    renderGutter();
  });
  editor.addEventListener("scroll", syncGutterScroll);

  // When the window's close button is hit and any tab is dirty, the Rust
  // handler prevents the close and fires this event. Ask the user; if they
  // confirm, call back into Rust to flip the force-close flag and re-close.
  listen("fedit:close-blocked", async () => {
    const confirmed = await ask("One or more tabs have unsaved changes. Close anyway?", {
      title: "fedit",
      kind: "warning",
    });
    if (confirmed) {
      await invoke("force_close_window");
    }
  });

  listen("fedit:menu-new", () => createNewTab());
  listen("fedit:menu-open", () => document.querySelector("#open-btn").click());
  listen("fedit:menu-save", () => document.querySelector("#save-btn").click());
  listen("fedit:menu-save-as", () => document.querySelector("#save-as-btn").click());
  listen("fedit:menu-close-tab", async () => {
    if (activeId === null) return;
    const tabs = await invoke("list_tabs");
    const t = tabs.find((t) => t.id === activeId);
    if (t) closeTab(t.id, t.dirty);
  });

  // --------------------------------------------------------------------------
  // Find / replace bar (ch22)
  // --------------------------------------------------------------------------
  const findBar      = document.querySelector("#find-bar");
  const findInput    = document.querySelector("#find-input");
  const replaceInput = document.querySelector("#replace-input");
  const findRegex    = document.querySelector("#find-regex");
  const findCase     = document.querySelector("#find-case");
  const findCount    = document.querySelector("#find-count");

  let findState = { matches: [], index: -1 };

  function openFindBar() {
    findBar.hidden = false;
    findInput.focus();
    findInput.select();
  }
  function closeFindBar() {
    findBar.hidden = true;
    findState = { matches: [], index: -1 };
    findCount.textContent = "";
    editor.focus();
  }

  async function runFind() {
    const needle = findInput.value;
    if (needle === "") {
      findState = { matches: [], index: -1 };
      findCount.textContent = "";
      return;
    }
    try {
      const matches = await invoke("find_matches", {
        haystack: editor.value,
        needle,
        useRegex: findRegex.checked,
        caseSensitive: findCase.checked,
      });
      findState = { matches, index: matches.length ? 0 : -1 };
      updateFindCount();
      revealCurrentMatch();
    } catch (err) {
      findCount.textContent = `regex: ${err?.message ?? err}`;
    }
  }

  function updateFindCount() {
    const { matches, index } = findState;
    if (matches.length === 0) {
      findCount.textContent = "no matches";
    } else {
      findCount.textContent = `${index + 1} / ${matches.length}`;
    }
  }

  function revealCurrentMatch() {
    const { matches, index } = findState;
    if (index < 0 || index >= matches.length) return;
    const m = matches[index];
    editor.focus();
    editor.setSelectionRange(m.start, m.end);
    const approxLine = editor.value.slice(0, m.start).split("\n").length;
    const lineHeight = parseFloat(getComputedStyle(editor).lineHeight) || 20;
    editor.scrollTop = Math.max(0, approxLine * lineHeight - editor.clientHeight / 2);
    syncGutterScroll();
  }

  function step(delta) {
    const { matches } = findState;
    if (matches.length === 0) return;
    findState.index = (findState.index + delta + matches.length) % matches.length;
    updateFindCount();
    revealCurrentMatch();
  }

  async function replaceAll() {
    const needle = findInput.value;
    if (needle === "") return;
    try {
      const newText = await invoke("replace_matches", {
        haystack: editor.value,
        needle,
        replacement: replaceInput.value,
        useRegex: findRegex.checked,
        caseSensitive: findCase.checked,
      });
      if (newText !== editor.value) {
        editor.value = newText;
        buffers.set(activeId, newText);
        renderGutter();
        if (dirtyDot.hidden) {
          dirtyDot.hidden = false;
          await invoke("set_tab_dirty", { id: activeId, dirty: true });
          await refreshTabs();
        }
      }
      await runFind();
    } catch (err) {
      findCount.textContent = `regex: ${err?.message ?? err}`;
    }
  }

  document.querySelector("#find-btn").addEventListener("click", openFindBar);
  document.querySelector("#find-close-btn").addEventListener("click", closeFindBar);
  document.querySelector("#find-next-btn").addEventListener("click", () => step(+1));
  document.querySelector("#find-prev-btn").addEventListener("click", () => step(-1));
  document.querySelector("#replace-all-btn").addEventListener("click", replaceAll);
  findInput.addEventListener("input", runFind);
  findRegex.addEventListener("change", runFind);
  findCase.addEventListener("change", runFind);
  findInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); step(e.shiftKey ? -1 : +1); }
    if (e.key === "Escape") { e.preventDefault(); closeFindBar(); }
  });
  document.addEventListener("keydown", (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key === "f") { e.preventDefault(); openFindBar(); }
  });
});
