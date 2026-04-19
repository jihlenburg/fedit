const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

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

  const pathView = document.querySelector("#path-view");
  const editor = document.querySelector("#editor");

  async function refreshPath() {
    const path = await invoke("current_path");
    pathView.textContent = path ?? "";
    return path;
  }
  refreshPath();

  document.querySelector("#open-btn").addEventListener("click", async () => {
    const path = await open({ multiple: false, directory: false });
    if (path === null) {
      return;
    }
    try {
      editor.value = await invoke("read_file", { path });
      await refreshPath();
    } catch (err) {
      pathView.textContent = `${path} — error: ${err}`;
    }
  });

  document.querySelector("#save-btn").addEventListener("click", async () => {
    const path = await invoke("current_path");
    if (path === null) {
      pathView.textContent = "open a file first";
      return;
    }
    try {
      await invoke("write_file", { path, contents: editor.value });
      pathView.textContent = `${path} — saved`;
    } catch (err) {
      pathView.textContent = `${path} — error: ${err}`;
    }
  });
});
