const tauri = window.__TAURI__;
const internals = window.__TAURI_INTERNALS__;

function resolveInvoke() {
  const candidates = [
    tauri?.core?.invoke,
    tauri?.invoke,
    internals?.invoke,
    window.__TAURI_INVOKE__,
  ];

  for (const candidate of candidates) {
    if (typeof candidate === "function") {
      return candidate;
    }
  }

  return null;
}

function resolveListen() {
  const candidates = [tauri?.event?.listen, tauri?.listen];
  for (const candidate of candidates) {
    if (typeof candidate === "function") {
      return candidate;
    }
  }
  return null;
}

const invokeRaw = resolveInvoke();
const listen = resolveListen();

async function invoke(command, args = {}) {
  if (typeof invokeRaw !== "function") {
    throw new Error("bridge_invoke_unavailable");
  }
  return invokeRaw(command, args);
}

const eventsBuffer = [];
let refreshTimer = null;

function beep() {
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const oscillator = ctx.createOscillator();
    const gain = ctx.createGain();

    oscillator.type = "sine";
    oscillator.frequency.value = 880;
    gain.gain.value = 0.08;

    oscillator.connect(gain);
    gain.connect(ctx.destination);
    oscillator.start();
    oscillator.stop(ctx.currentTime + 0.1);
  } catch (_) {
    // ignore if audio context is blocked
  }
}

function pushEvent(entry) {
  const line = `[${new Date().toLocaleTimeString()}] ${entry}`;
  eventsBuffer.unshift(line);
  if (eventsBuffer.length > 40) {
    eventsBuffer.pop();
  }
  document.getElementById("events").textContent = eventsBuffer.join("\n");
}

function bridgeDebugInfo() {
  return {
    has___TAURI__: Boolean(tauri),
    tauri_keys: tauri ? Object.keys(tauri) : [],
    has___TAURI_INTERNALS__: Boolean(internals),
    internals_keys: internals ? Object.keys(internals) : [],
    invoke_type: typeof invokeRaw,
    listen_type: typeof listen,
  };
}

async function refresh() {
  if (typeof invokeRaw !== "function") {
    const debug = bridgeDebugInfo();
    const text =
      "Puente Tauri no disponible.\n\n" +
      "Diagnóstico:\n" +
      JSON.stringify(debug, null, 2);

    document.getElementById("settings").textContent = text;
    document.getElementById("stats").textContent = text;
    document.getElementById("runtime-status").textContent = text;
    return;
  }

  const [settings, stats, runtime] = await Promise.all([
    invoke("get_settings"),
    invoke("get_weekly_stats"),
    invoke("get_runtime_status"),
  ]);

  document.getElementById("settings").textContent = JSON.stringify(settings, null, 2);
  document.getElementById("stats").textContent = JSON.stringify(stats, null, 2);
  document.getElementById("runtime-status").textContent = JSON.stringify(runtime, null, 2);
}

async function withAction(name, action) {
  try {
    await action();
    pushEvent(`OK: ${name}`);
  } catch (err) {
    pushEvent(`ERROR en ${name}: ${String(err)}`);
  }
  await refresh();
}

document.getElementById("refresh").addEventListener("click", refresh);

document.getElementById("runtime-start").addEventListener("click", () =>
  withAction("iniciar runtime", () => invoke("start_runtime"))
);

document.getElementById("runtime-stop").addEventListener("click", () =>
  withAction("detener runtime", () => invoke("stop_runtime"))
);

document.getElementById("start-pending").addEventListener("click", () =>
  withAction("iniciar descanso pendiente", () => invoke("start_pending_break"))
);

document.getElementById("snooze-pending").addEventListener("click", () =>
  withAction("posponer descanso pendiente", () => invoke("snooze_pending_break"))
);

document.getElementById("trigger-micro").addEventListener("click", () =>
  withAction("forzar micro", () => invoke("trigger_break", { kind: "micro" }))
);

document.getElementById("trigger-rest").addEventListener("click", () =>
  withAction("forzar descanso", () => invoke("trigger_break", { kind: "rest" }))
);

document.getElementById("strict").addEventListener("click", async () => {
  if (typeof invokeRaw !== "function") return;
  await withAction("modo estricto", async () => {
    const settings = await invoke("get_settings");
    settings.block_level = "strict";
    await invoke("update_settings", { settings });
  });
});

document.getElementById("xdg").addEventListener("click", () =>
  withAction("autoarranque xdg", () =>
    invoke("set_startup_mode", { mode: "xdg_only" })
  )
);

document.getElementById("both").addEventListener("click", () =>
  withAction("autoarranque xdg+systemd", () =>
    invoke("set_startup_mode", { mode: "xdg_and_systemd" })
  )
);

if (typeof listen === "function") {
  try {
    listen("runtime://event", async (event) => {
      const payload = event.payload || {};
      const label = `${payload.kind || "evento"}: ${payload.message || ""}`;
      pushEvent(label.trim());

      if (payload.kind === "break_due" || payload.kind === "break_started") {
        beep();
      }

      if (payload.kind === "break_tick" || payload.kind === "break_completed") {
        await refresh();
      }
    });
  } catch (err) {
    pushEvent(`WARN: listener no disponible (${String(err)})`);
  }
} else {
  pushEvent("INFO: sin listener de eventos; usando refresco periódico.");
}

if (!refreshTimer) {
  refreshTimer = setInterval(() => {
    refresh().catch((err) => {
      pushEvent(`WARN refresh: ${String(err)}`);
    });
  }, 2000);
}

refresh().catch((err) => {
  pushEvent(`ERROR inicial: ${String(err)}`);
});
