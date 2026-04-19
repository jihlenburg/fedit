const { invoke } = window.__TAURI__.core;

// Every OS ships a `hosts` file in a different place. Pick the right one so this
// demo works on Windows 10/11 as well as macOS and Linux.
const HARDCODED_PATH = /Windows/i.test(navigator.userAgent)
  ? "C:\\Windows\\System32\\drivers\\etc\\hosts"
  : "/etc/hosts";

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

  const fileView = document.querySelector("#file-view");
  document.querySelector("#load-btn").addEventListener("click", async () => {
    try {
      fileView.textContent = await invoke("read_file", { path: HARDCODED_PATH });
    } catch (err) {
      fileView.textContent = `error: ${err}`;
    }
  });
});
