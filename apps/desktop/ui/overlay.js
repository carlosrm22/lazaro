const tauri = window.__TAURI__;
const listen = tauri?.event?.listen;

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

if (listen) {
  listen("runtime://event", (event) => {
    const payload = event.payload || {};

    if (payload.break_kind) {
      kindNode.textContent = `Tipo: ${payload.break_kind}`;
    }

    if (typeof payload.remaining_seconds === "number") {
      remainingNode.textContent = formatSeconds(payload.remaining_seconds);
    }

    if (payload.kind === "break_started") {
      beep();
    }
  });
}
