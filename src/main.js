const { invoke } = window.__TAURI__.core;
const { open, save: saveDialog } = window.__TAURI__.dialog;
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
  let localDirty = false;

  // Render line numbers into the gutter, one per line in the textarea.
  // `split("\n").length` is how many lines exist — even an empty buffer is "1".
  function renderGutter() {
    const lineCount = editor.value.length === 0 ? 1 : editor.value.split("\n").length;
    const nums = new Array(lineCount);
    for (let i = 0; i < lineCount; i++) nums[i] = i + 1;
    gutter.textContent = nums.join("\n");
  }

  // Keep the gutter's scroll pinned to the textarea's scroll position.
  // overflow:hidden on .gutter means it never shows a scrollbar — we drive it.
  function syncGutterScroll() {
    gutter.scrollTop = editor.scrollTop;
  }

  async function refreshPath() {
    const path = await invoke("current_path");
    pathText.textContent = path ?? "";
    return path;
  }

  function setDirty(next) {
    localDirty = next;
    dirtyDot.hidden = !next;
    return invoke("set_dirty", { dirty: next });
  }

  async function doSave(newPath) {
    try {
      const saved = await invoke("save", {
        newPath: newPath ?? null,
        contents: editor.value,
      });
      await setDirty(false);
      pathText.textContent = `${saved} — saved`;
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
        li.addEventListener("click", async () => {
          try {
            editor.value = await invoke("read_file", { path: p });
            renderGutter();
            await refreshPath();
            await setDirty(false);
            await refreshRecent();
          } catch (err) {
            pathText.textContent = `${p} — error: ${err?.message ?? err}`;
          }
        });
        return li;
      }),
    );
  }

  refreshPath();
  refreshRecent();
  renderGutter();

  document.querySelector("#open-btn").addEventListener("click", async () => {
    const path = await open({ multiple: false, directory: false });
    if (path === null) {
      return;
    }
    try {
      editor.value = await invoke("read_file", { path });
      renderGutter();
      await refreshPath();
      await setDirty(false);
      await refreshRecent();
    } catch (err) {
      pathText.textContent = `${path} — error: ${err?.message ?? err}`;
    }
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

  editor.addEventListener("input", () => {
    if (!localDirty) {
      setDirty(true);
    }
    renderGutter();
  });
  editor.addEventListener("scroll", syncGutterScroll);

  listen("fedit:close-blocked", () => {
    pathText.textContent = "unsaved changes — save before closing";
  });

  async function newFile() {
    editor.value = "";
    pathText.textContent = "";
    renderGutter();
    await setDirty(false);
  }

  listen("fedit:menu-new", newFile);
  listen("fedit:menu-open", () => document.querySelector("#open-btn").click());
  listen("fedit:menu-save", () => document.querySelector("#save-btn").click());
  listen("fedit:menu-save-as", () => document.querySelector("#save-as-btn").click());

  // --------------------------------------------------------------------------
  // Find / replace bar (ch22)
  //
  // The Rust side owns the search — it compiles the regex and returns every
  // match as a (start, end) byte range. JS turns those ranges into a textarea
  // selection via setSelectionRange. Keeping the regex compile in Rust means
  // we get one correct error message and no regex engine divergence between
  // browsers.
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
    // Nudge the textarea to scroll the selection into view by temporarily
    // replacing the value with the same thing — blurring+refocusing is the
    // widely-used trick but sets dirty; instead we use scrollTop heuristic.
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
        renderGutter();
        await setDirty(true);
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
