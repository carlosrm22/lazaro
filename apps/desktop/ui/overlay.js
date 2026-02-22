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

const kindNode = document.getElementById("kind");
const remainingNode = document.getElementById("remaining");

function formatSeconds(seconds) {
  const s = Math.max(0, Number(seconds || 0));
  const mm = String(Math.floor(s / 60)).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return `${mm}:${ss}`;
}

function beep() {
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const oscillator = ctx.createOscillator();
    const gain = ctx.createGain();

    oscillator.type = "triangle";
    oscillator.frequency.value = 660;
    gain.gain.value = 0.08;

    oscillator.connect(gain);
    gain.connect(ctx.destination);
    oscillator.start();
    oscillator.stop(ctx.currentTime + 0.15);
  } catch (_) {
    // ignore
  }
}

function updateFromPayload(payload) {
  if (payload.break_kind) {
    kindNode.textContent = `Tipo: ${payload.break_kind}`;
  }

  if (typeof payload.remaining_seconds === "number") {
    remainingNode.textContent = formatSeconds(payload.remaining_seconds);
  }
}

if (typeof listen === "function") {
  try {
    listen("runtime://event", (event) => {
      const payload = event.payload || {};
      updateFromPayload(payload);

      if (payload.kind === "break_started") {
        beep();
      }
    });
  } catch (_) {
    // fallback to polling below
  }
}

if (typeof invokeRaw === "function") {
  setInterval(async () => {
    try {
      const runtime = await invoke("get_runtime_status");
      updateFromPayload(runtime);
    } catch (_) {
      // ignore polling issues
    }
  }, 1000);
}
