const invoke = window.__TAURI__?.core?.invoke;

async function refresh() {
  if (!invoke) {
    document.getElementById("settings").textContent =
      "Tauri invoke bridge unavailable. Run inside Lazaro desktop runtime.";
    return;
  }

  const settings = await invoke("get_settings");
  document.getElementById("settings").textContent = JSON.stringify(settings, null, 2);
}

document.getElementById("refresh").addEventListener("click", refresh);

document.getElementById("strict").addEventListener("click", async () => {
  if (!invoke) return;
  const settings = await invoke("get_settings");
  settings.block_level = "strict";
  await invoke("update_settings", { settings });
  await refresh();
});

document.getElementById("xdg").addEventListener("click", async () => {
  if (!invoke) return;
  await invoke("set_startup_mode", { mode: "xdg_only" });
  await refresh();
});

document.getElementById("both").addEventListener("click", async () => {
  if (!invoke) return;
  await invoke("set_startup_mode", { mode: "xdg_and_systemd" });
  await refresh();
});

refresh();
