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
  const fileView = document.querySelector("#file-view");
  document.querySelector("#open-btn").addEventListener("click", async () => {
    const path = await open({ multiple: false, directory: false });
    if (path === null) {
      return;
    }
    pathView.textContent = path;
    try {
      fileView.textContent = await invoke("read_file", { path });
    } catch (err) {
      fileView.textContent = `error: ${err}`;
    }
  });
});
