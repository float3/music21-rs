// @ts-nocheck
const systemsNode = document.querySelector("#systems");
const summary = document.querySelector("#summary");
const systemTitle = document.querySelector("#system-title");
const scaleStrip = document.querySelector("#scale-strip");
const degreesBody = document.querySelector("#degrees");
const controls = document.querySelector("#controls");
const rootFrequency = document.querySelector("#root-frequency");
const tempoInput = document.querySelector("#tempo");
const waveform = document.querySelector("#waveform");
const stopButton = document.querySelector("#stop");
const error = document.querySelector("#error");
const docsLink = document.querySelector("#docs-link");

const isLocalExample = ["127.0.0.1", "localhost"].includes(window.location.hostname)
  || window.location.protocol === "file:";
docsLink.href = isLocalExample ? "../docs/music21_rs/index.html" : "../docs/music21_rs/index.html";

let tuningSystems = [];
let selectedSystemId = "";
let selectedDegree = 0;
let audioContext = null;
let activeNodes = [];
let activeTimers = [];

function showError(message) {
  error.textContent = message;
  error.style.display = "block";
}

function clearError() {
  error.style.display = "none";
}

function clampNumber(value, min, max, fallback) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

function selectedSystem() {
  return tuningSystems.find((system) => system.id === selectedSystemId) ?? tuningSystems[0];
}

async function loadTuningSystems() {
  const candidates = ["../pkg/music21_rs_web.js"];
  let lastError = null;
  for (const candidate of candidates) {
    try {
      const module = await import(new URL(candidate, window.location.href).href);
      await module.default();
      if (typeof module.tuning_systems !== "function") {
        throw new Error("The current WASM package does not expose tuning_systems yet.");
      }
      const frequency = clampNumber(rootFrequency.value, 20, 2000, 261.6256);
      rootFrequency.value = frequency;
      tuningSystems = module.tuning_systems(frequency);
      if (!selectedSystemId || !tuningSystems.some((system) => system.id === selectedSystemId)) {
        selectedSystemId = tuningSystems[0]?.id ?? "";
      }
      clearError();
      render();
      return;
    } catch (err) {
      lastError = err;
    }
  }
  showError(lastError instanceof Error ? lastError.message : String(lastError));
}

function render() {
  renderSystemList();
  renderSelectedSystem();
}

function renderSystemList() {
  systemsNode.replaceChildren();
  for (const system of tuningSystems) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `system-button${system.id === selectedSystemId ? " active" : ""}`;
    const name = document.createElement("strong");
    name.textContent = system.name;
    const meta = document.createElement("span");
    meta.textContent = `${system.octave_size} degrees per octave`;
    const description = document.createElement("small");
    description.textContent = system.description;
    button.append(name, meta, description);
    button.addEventListener("click", () => {
      stopPlayback();
      selectedSystemId = system.id;
      selectedDegree = 0;
      render();
    });
    systemsNode.appendChild(button);
  }
}

function renderSelectedSystem() {
  const system = selectedSystem();
  if (!system) return;
  systemTitle.textContent = system.name;
  renderSummary(system);
  renderScale(system);
  renderDegrees(system);
}

function renderSummary(system) {
  const rows = [
    ["System", system.id],
    ["Octave size", system.octave_size],
    ["Root", `${formatFrequency(system.root_frequency_hz)} Hz`],
    ["Scale length", `${system.degrees.length} notes`],
  ];
  summary.replaceChildren();
  for (const [label, value] of rows) {
    const item = document.createElement("div");
    item.className = "fact";
    const labelNode = document.createElement("span");
    labelNode.textContent = label;
    const valueNode = document.createElement("strong");
    valueNode.textContent = value;
    item.append(labelNode, valueNode);
    summary.appendChild(item);
  }
}

function renderScale(system) {
  scaleStrip.replaceChildren();
  for (const degree of system.degrees) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `degree-button${degree.degree === selectedDegree ? " active" : ""}`;
    const label = document.createElement("strong");
    label.textContent = degree.label;
    const frequency = document.createElement("small");
    frequency.textContent = `${formatFrequency(degree.frequency_hz)} Hz`;
    button.append(label, frequency);
    button.addEventListener("click", () => {
      selectedDegree = degree.degree;
      renderScale(system);
      playDegree(degree);
    });
    scaleStrip.appendChild(button);
  }

  const playButton = document.createElement("button");
  playButton.type = "button";
  playButton.className = "degree-button active";
  playButton.innerHTML = "<strong>Play scale</strong><small>all degrees</small>";
  playButton.addEventListener("click", () => playScale(system));
  scaleStrip.appendChild(playButton);
}

function renderDegrees(system) {
  degreesBody.replaceChildren();
  for (const degree of system.degrees) {
    const row = document.createElement("tr");
    for (const value of [
      degree.degree,
      degree.label,
      degree.ratio_label,
      `${formatFrequency(degree.frequency_hz)} Hz`,
    ]) {
      const cell = document.createElement("td");
      cell.textContent = value;
      row.appendChild(cell);
    }

    const centsCell = document.createElement("td");
    centsCell.appendChild(renderCents(degree.cents_from_equal_temperament));
    row.appendChild(centsCell);

    const playCell = document.createElement("td");
    const play = document.createElement("button");
    play.type = "button";
    play.className = "secondary";
    play.textContent = "Play";
    play.addEventListener("click", () => {
      selectedDegree = degree.degree;
      renderScale(system);
      playDegree(degree);
    });
    playCell.appendChild(play);
    row.appendChild(playCell);
    degreesBody.appendChild(row);
  }
}

function renderCents(cents) {
  const wrapper = document.createElement("div");
  wrapper.className = "cents";
  const value = document.createElement("span");
  value.className = "cents-value";
  value.textContent = formatCents(cents);
  const track = document.createElement("span");
  track.className = "cents-track";
  const fill = document.createElement("span");
  fill.className = `cents-fill${cents < 0 ? " negative" : ""}`;
  fill.style.width = `${Math.min(100, Math.abs(cents) * 3)}%`;
  track.appendChild(fill);
  wrapper.append(value, track);
  return wrapper;
}

function formatFrequency(value) {
  return Number(value).toFixed(value >= 100 ? 2 : 3);
}

function formatCents(value) {
  const rounded = Number(value).toFixed(2);
  return value > 0 ? `+${rounded}` : rounded;
}

async function ensureAudio() {
  const AudioContextConstructor = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextConstructor) {
    showError("Audio is not available in this browser.");
    return false;
  }
  audioContext ||= new AudioContextConstructor();
  if (audioContext.state === "suspended") {
    await audioContext.resume();
  }
  return true;
}

function noteDurationSeconds() {
  const tempo = clampNumber(tempoInput.value, 40, 320, 150);
  tempoInput.value = tempo;
  return 60 / tempo;
}

async function playDegree(degree) {
  if (!await ensureAudio()) return;
  stopPlayback();
  scheduleTone(degree.frequency_hz, audioContext.currentTime, Math.max(0.24, noteDurationSeconds() * 0.9), true);
}

async function playScale(system) {
  if (!await ensureAudio()) return;
  stopPlayback();
  const duration = noteDurationSeconds();
  const start = audioContext.currentTime + 0.04;
  system.degrees.forEach((degree, index) => {
    const timer = window.setTimeout(() => {
      selectedDegree = degree.degree;
      renderScale(system);
    }, index * duration * 1000);
    activeTimers.push(timer);
    scheduleTone(degree.frequency_hz, start + index * duration, duration * 0.86, index === 0);
  });
}

function scheduleTone(frequency, startTime, duration, accent = false) {
  const oscillator = audioContext.createOscillator();
  const gain = audioContext.createGain();
  oscillator.type = waveform.value;
  oscillator.frequency.setValueAtTime(frequency, startTime);
  gain.gain.setValueAtTime(0.0001, startTime);
  gain.gain.exponentialRampToValueAtTime(accent ? 0.32 : 0.22, startTime + 0.015);
  gain.gain.exponentialRampToValueAtTime(0.0001, startTime + duration);
  oscillator.connect(gain).connect(audioContext.destination);
  oscillator.start(startTime);
  oscillator.stop(startTime + duration + 0.08);
  activeNodes.push(oscillator);
  oscillator.addEventListener("ended", () => {
    activeNodes = activeNodes.filter((node) => node !== oscillator);
  });
}

function stopPlayback() {
  for (const timer of activeTimers) {
    window.clearTimeout(timer);
  }
  activeTimers = [];
  for (const node of activeNodes) {
    try {
      node.stop();
    } catch {
      // Already stopped.
    }
  }
  activeNodes = [];
}

controls.addEventListener("submit", (event) => {
  event.preventDefault();
  stopPlayback();
  loadTuningSystems();
});
stopButton.addEventListener("click", stopPlayback);
rootFrequency.addEventListener("change", loadTuningSystems);

loadTuningSystems();
