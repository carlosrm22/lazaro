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
  profiles: [],
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

const timeFields = new Set([
  "micro_interval_seconds",
  "micro_duration_seconds",
  "micro_snooze_seconds",
  "rest_interval_seconds",
  "rest_duration_seconds",
  "rest_snooze_seconds",
  "daily_limit_seconds",
  "daily_limit_snooze_seconds",
]);

function unitSelectId(fieldId) {
  return `${fieldId}__unit`;
}

function normalizeNumber(value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return 0;
  }
  return Math.max(0, parsed);
}

function preferredUnitFromSeconds(seconds) {
  const value = Math.max(0, Number(seconds || 0));
  if (value >= 60 && value % 60 === 0) {
    return "minutes";
  }
  return "seconds";
}

function formatNumberForInput(value) {
  if (!Number.isFinite(value)) {
    return "0";
  }
  if (Number.isInteger(value)) {
    return String(value);
  }
  return String(Math.round(value * 100) / 100);
}

function secondsToDisplay(seconds, unit) {
  if (unit === "minutes") {
    return Number(seconds || 0) / 60;
  }
  return Number(seconds || 0);
}

function displayToSeconds(value, unit) {
  const normalized = normalizeNumber(value);
  const seconds = unit === "minutes" ? normalized * 60 : normalized;
  return Math.round(seconds);
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

function normalizeProfileId(value) {
  const normalized = (value || "")
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized || "perfil";
}

function uniqueProfileId(baseId) {
  const ids = new Set((state.profiles || []).map((profile) => profile.id));
  if (!ids.has(baseId)) {
    return baseId;
  }

  let index = 2;
  while (ids.has(`${baseId}-${index}`)) {
    index += 1;
  }
  return `${baseId}-${index}`;
}

function getSelectedProfile() {
  const select = document.getElementById("profile-select");
  const id = select?.value;
  return state.profiles.find((profile) => profile.id === id) || null;
}

function getProfileById(id) {
  return state.profiles.find((profile) => profile.id === id) || null;
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
  let nextBreakIn = "-";
  if (runtime.running) {
    if (runtime.active_break) {
      nextBreakIn = "descanso en curso";
    } else if (runtime.next_break_seconds != null) {
      nextBreakIn =
        runtime.next_break_seconds === 0
          ? "ahora"
          : formatSeconds(runtime.next_break_seconds);
    }
  }

  const entries = [
    ["running", runtime.running ? "sí" : "no"],
    ["pendiente", runtime.pending_break || "ninguno"],
    ["en descanso", runtime.active_break || "ninguno"],
    ["restante", runtime.remaining_seconds != null ? formatSeconds(runtime.remaining_seconds) : "-"],
    ["próximo tipo", runtime.next_break_kind || "-"],
    ["próximo descanso en", nextBreakIn],
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

function renderProfiles() {
  const select = document.getElementById("profile-select");
  select.innerHTML = "";

  const sorted = [...state.profiles].sort((a, b) => a.name.localeCompare(b.name));
  for (const profile of sorted) {
    const option = document.createElement("option");
    option.value = profile.id;
    option.textContent = `${profile.name} (${profile.id})`;
    if (state.settings?.active_profile_id === profile.id) {
      option.selected = true;
    }
    select.appendChild(option);
  }

  if (!select.value && sorted.length > 0) {
    select.value = sorted[0].id;
  }
}

function renderSettingsForm() {
  if (!state.settings) return;

  for (const key of settingsFields) {
    const element = document.getElementById(key);
    if (!element) continue;

    const value = state.settings[key];
    if (element.type === "checkbox") {
      element.checked = Boolean(value);
    } else if (element.type === "number" && timeFields.has(key)) {
      const unitSelect = document.getElementById(unitSelectId(key));
      const unit = preferredUnitFromSeconds(value);
      if (unitSelect) {
        unitSelect.value = unit;
        unitSelect.dataset.prevUnit = unit;
      }
      element.value = formatNumberForInput(secondsToDisplay(value, unit));
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
      if (timeFields.has(key)) {
        const unitSelect = document.getElementById(unitSelectId(key));
        const unit = unitSelect?.value === "minutes" ? "minutes" : "seconds";
        next[key] = displayToSeconds(element.value, unit);
      } else {
        next[key] = Number(element.value || 0);
      }
      continue;
    }

    next[key] = element.value;
  }

  if (!next.active_profile_id) {
    next.active_profile_id = state.settings?.active_profile_id || "default";
  }

  return next;
}

function setupUnitSelectors() {
  for (const field of timeFields) {
    const select = document.getElementById(unitSelectId(field));
    const input = document.getElementById(field);
    if (!select || !input) continue;

    select.addEventListener("change", () => {
      const prevUnit = select.dataset.prevUnit || "seconds";
      const nextUnit = select.value === "minutes" ? "minutes" : "seconds";
      if (prevUnit === nextUnit) {
        return;
      }

      const currentValue = normalizeNumber(input.value);
      const seconds = prevUnit === "minutes" ? currentValue * 60 : currentValue;
      const converted = nextUnit === "minutes" ? seconds / 60 : seconds;

      input.value = formatNumberForInput(converted);
      select.dataset.prevUnit = nextUnit;
    });
  }
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
      profiles: state.profiles,
      stats: state.stats,
      runtime: state.runtime,
    },
    null,
    2
  );
}

function renderAll() {
  renderRuntime();
  renderProfiles();
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

  const [settings, stats, runtime, profiles] = await Promise.all([
    invoke("get_settings"),
    invoke("get_weekly_stats"),
    invoke("get_runtime_status"),
    invoke("list_profiles"),
  ]);

  state.settings = settings;
  state.stats = stats;
  state.runtime = runtime;
  state.profiles = profiles || [];
  renderAll();
}

async function syncActiveProfileFromSettings() {
  if (!state.settings) return;
  const profile = getProfileById(state.settings.active_profile_id);
  if (!profile) return;

  await invoke("save_profile", {
    profile: {
      id: profile.id,
      name: profile.name,
      settings: state.settings,
    },
  });
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
    await syncActiveProfileFromSettings();
  });
});

document.getElementById("save-settings").addEventListener("click", async () => {
  await withAction("guardar ajustes", async () => {
    const next = collectSettingsFromForm();
    state.settings = await invoke("update_settings", { settings: next });
    await syncActiveProfileFromSettings();

    const startupMode = next.startup_systemd_user ? "xdg_and_systemd" : "xdg_only";
    await invoke("set_startup_mode", { mode: startupMode });
  });
});

document.getElementById("activate-profile").addEventListener("click", async () => {
  await withAction("activar perfil", async () => {
    const selected = document.getElementById("profile-select").value;
    if (!selected) {
      throw new Error("selecciona un perfil");
    }

    await invoke("activate_profile", { profileId: selected });
  });
});

document.getElementById("create-profile").addEventListener("click", async () => {
  await withAction("crear perfil", async () => {
    const profileName = document.getElementById("profile-name").value.trim();
    if (!profileName) {
      throw new Error("escribe un nombre de perfil");
    }

    const baseId = normalizeProfileId(profileName);
    const id = uniqueProfileId(baseId);

    const settings = collectSettingsFromForm();
    settings.active_profile_id = id;

    await invoke("save_profile", {
      profile: {
        id,
        name: profileName,
        settings,
      },
    });

    await invoke("activate_profile", { profileId: id });
    document.getElementById("profile-name").value = "";
  });
});

document.getElementById("duplicate-profile").addEventListener("click", async () => {
  await withAction("duplicar perfil", async () => {
    const selected = getSelectedProfile();
    if (!selected) {
      throw new Error("selecciona un perfil");
    }

    const customName = document.getElementById("profile-name").value.trim();
    const profileName = customName || `${selected.name} copia`;
    const id = uniqueProfileId(normalizeProfileId(profileName));

    const duplicatedSettings = { ...selected.settings, active_profile_id: id };

    await invoke("save_profile", {
      profile: {
        id,
        name: profileName,
        settings: duplicatedSettings,
      },
    });

    await invoke("activate_profile", { profileId: id });
    document.getElementById("profile-name").value = "";
  });
});

document.getElementById("delete-profile").addEventListener("click", async () => {
  await withAction("eliminar perfil", async () => {
    const selected = document.getElementById("profile-select").value;
    if (!selected) {
      throw new Error("selecciona un perfil");
    }
    if (selected === "default") {
      throw new Error("no puedes eliminar el perfil default");
    }

    await invoke("remove_profile", { profileId: selected });
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

setupUnitSelectors();
refresh().catch((err) => pushEvent("error", `error inicial: ${String(err)}`));
