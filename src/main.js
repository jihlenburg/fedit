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
  let localDirty = false;

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

  document.querySelector("#open-btn").addEventListener("click", async () => {
    const path = await open({ multiple: false, directory: false });
    if (path === null) {
      return;
    }
    try {
      editor.value = await invoke("read_file", { path });
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
  });

  listen("fedit:close-blocked", () => {
    pathText.textContent = "unsaved changes — save before closing";
  });

  async function newFile() {
    editor.value = "";
    pathText.textContent = "";
    await setDirty(false);
  }

  listen("fedit:menu-new", newFile);
  listen("fedit:menu-open", () => document.querySelector("#open-btn").click());
  listen("fedit:menu-save", () => document.querySelector("#save-btn").click());
  listen("fedit:menu-save-as", () => document.querySelector("#save-as-btn").click());
});
