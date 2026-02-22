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

const state = {
  settings: null,
  stats: null,
  runtime: null,
  events: [],
  refreshTimer: null,
  showDebug: false,
};

const settingsFields = [
  "micro_interval_seconds",
  "micro_duration_seconds",
  "micro_snooze_seconds",
  "rest_interval_seconds",
  "rest_duration_seconds",
  "rest_snooze_seconds",
  "daily_limit_seconds",
  "daily_limit_snooze_seconds",
  "daily_reset_time",
  "block_level",
  "desktop_notifications",
  "overlay_notifications",
  "sound_notifications",
  "sound_theme",
  "startup_xdg",
  "startup_systemd_user",
  "active_profile_id",
];

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

function formatSeconds(totalSeconds) {
  const seconds = Number(totalSeconds || 0);
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  if (h > 0) {
    return `${h}h ${m}m ${s}s`;
  }
  if (m > 0) {
    return `${m}m ${s}s`;
  }
  return `${s}s`;
}

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
    // ignore if blocked
  }
}

function pushEvent(kind, message) {
  const item = {
    kind,
    message,
    time: new Date().toLocaleTimeString(),
  };

  state.events.unshift(item);
  if (state.events.length > 80) {
    state.events.pop();
  }
  renderEvents();
}

function renderEvents() {
  const node = document.getElementById("events-list");
  node.innerHTML = "";

  if (state.events.length === 0) {
    const li = document.createElement("li");
    li.className = "event-item";
    li.textContent = "Sin eventos todavía.";
    node.appendChild(li);
    return;
  }

  for (const event of state.events) {
    const li = document.createElement("li");
    li.className = `event-item ${event.kind || "info"}`;

    const meta = document.createElement("div");
    meta.className = "meta";

    const left = document.createElement("span");
    left.textContent = (event.kind || "evento").replaceAll("_", " ");

    const right = document.createElement("span");
    right.textContent = event.time;

    meta.appendChild(left);
    meta.appendChild(right);

    const msg = document.createElement("div");
    msg.textContent = event.message;

    li.appendChild(meta);
    li.appendChild(msg);
    node.appendChild(li);
  }
}

function renderRuntime() {
  const runtime = state.runtime || {};
  const container = document.getElementById("runtime-grid");

  const entries = [
    ["running", runtime.running ? "sí" : "no"],
    ["pendiente", runtime.pending_break || "ninguno"],
    ["en descanso", runtime.active_break || "ninguno"],
    ["restante", runtime.remaining_seconds != null ? `${runtime.remaining_seconds}s` : "-"],
    ["modo estricto", runtime.strict_mode ? "sí" : "no"],
    ["último evento", runtime.last_event || "-"]
  ];

  container.innerHTML = "";
  for (const [label, value] of entries) {
    const article = document.createElement("article");
    article.className = "runtime-item";

    const span = document.createElement("span");
    span.textContent = label;

    const strong = document.createElement("strong");
    strong.textContent = String(value);

    article.appendChild(span);
    article.appendChild(strong);
    container.appendChild(article);
  }

  const pill = document.getElementById("runtime-pill");
  pill.textContent = runtime.running ? "activo" : "detenido";
  pill.classList.toggle("running", Boolean(runtime.running));
}

function renderSettingsForm() {
  if (!state.settings) return;

  for (const key of settingsFields) {
    const element = document.getElementById(key);
    if (!element) continue;

    const value = state.settings[key];
    if (element.type === "checkbox") {
      element.checked = Boolean(value);
    } else {
      element.value = value ?? "";
    }
  }
}

function collectSettingsFromForm() {
  const next = { ...(state.settings || {}) };
  for (const key of settingsFields) {
    const element = document.getElementById(key);
    if (!element) continue;

    if (element.type === "checkbox") {
      next[key] = Boolean(element.checked);
      continue;
    }

    if (element.type === "number") {
      next[key] = Number(element.value || 0);
      continue;
    }

    next[key] = element.value;
  }

  return next;
}

function renderAnalytics() {
  const stats = state.stats || {};
  const settings = state.settings || {};

  document.getElementById("metric-active").textContent = formatSeconds(stats.total_active_seconds);
  document.getElementById("metric-micro").textContent = String(stats.micro_done ?? 0);
  document.getElementById("metric-rest").textContent = String(stats.rest_done ?? 0);
  document.getElementById("metric-daily").textContent = String(stats.daily_limit_hits ?? 0);
  document.getElementById("metric-skipped").textContent = String(stats.skipped ?? 0);

  const weeklyTarget = Math.max(1, Number(settings.daily_limit_seconds || 0) * 7);
  const percent = Math.min(100, Math.round(((Number(stats.total_active_seconds || 0)) / weeklyTarget) * 100));

  document.getElementById("progress-text").textContent = `${percent}%`;
  document.getElementById("progress-bar").style.width = `${percent}%`;
  document.getElementById("analytics-summary").textContent = `objetivo: ${formatSeconds(weeklyTarget)}`;
}

function renderDebug() {
  const node = document.getElementById("debug-json");
  node.classList.toggle("hidden", !state.showDebug);
  node.textContent = JSON.stringify(
    {
      bridge: bridgeDebugInfo(),
      settings: state.settings,
      stats: state.stats,
      runtime: state.runtime,
    },
    null,
    2
  );
}

function renderAll() {
  renderRuntime();
  renderSettingsForm();
  renderAnalytics();
  renderEvents();
  renderDebug();
}

async function refresh() {
  if (typeof invokeRaw !== "function") {
    const debug = bridgeDebugInfo();
    const text =
      "Puente Tauri no disponible.\\n\\n" +
      "Diagnóstico:\\n" +
      JSON.stringify(debug, null, 2);

    document.getElementById("runtime-grid").innerHTML = `<article class=\"runtime-item\"><strong>${text}</strong></article>`;
    document.getElementById("debug-json").textContent = text;
    return;
  }

  const [settings, stats, runtime] = await Promise.all([
    invoke("get_settings"),
    invoke("get_weekly_stats"),
    invoke("get_runtime_status"),
  ]);

  state.settings = settings;
  state.stats = stats;
  state.runtime = runtime;
  renderAll();
}

async function withAction(name, action) {
  try {
    await action();
    pushEvent("info", `OK: ${name}`);
  } catch (err) {
    pushEvent("error", `ERROR en ${name}: ${String(err)}`);
  }
  await refresh();
}

document.getElementById("refresh").addEventListener("click", () =>
  refresh().catch((err) => pushEvent("error", `ERROR refresh: ${String(err)}`))
);

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
  if (!state.settings) return;
  await withAction("modo estricto", async () => {
    const next = { ...state.settings, block_level: "strict" };
    state.settings = await invoke("update_settings", { settings: next });
  });
});

document.getElementById("save-settings").addEventListener("click", async () => {
  await withAction("guardar ajustes", async () => {
    const next = collectSettingsFromForm();
    state.settings = await invoke("update_settings", { settings: next });

    const startupMode = next.startup_systemd_user ? "xdg_and_systemd" : "xdg_only";
    await invoke("set_startup_mode", { mode: startupMode });
  });
});

document.getElementById("clear-events").addEventListener("click", () => {
  state.events = [];
  renderEvents();
});

document.getElementById("toggle-debug").addEventListener("click", () => {
  state.showDebug = !state.showDebug;
  renderDebug();
});

if (typeof listen === "function") {
  try {
    listen("runtime://event", async (event) => {
      const payload = event.payload || {};
      const kind = payload.kind || "info";
      const message = payload.message || "evento";
      pushEvent(kind, message);

      if (kind === "break_due" || kind === "break_started") {
        beep();
      }

      if (kind === "break_tick" || kind === "break_completed" || kind === "daily_reset") {
        await refresh();
      }
    });
  } catch (err) {
    pushEvent("warn", `listener no disponible (${String(err)})`);
  }
} else {
  pushEvent("warn", "sin listener de eventos; usando refresco periódico");
}

if (!state.refreshTimer) {
  state.refreshTimer = setInterval(() => {
    refresh().catch((err) => pushEvent("warn", `refresh: ${String(err)}`));
  }, 2000);
}

refresh().catch((err) => pushEvent("error", `error inicial: ${String(err)}`));
