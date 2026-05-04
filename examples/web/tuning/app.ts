// @ts-nocheck
import "../theme.js";

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
const shareButton = document.querySelector("#share");
const error = document.querySelector("#error");
const docsLink = document.querySelector("#docs-link");

docsLink.href = "../docs/music21_rs/index.html";

const defaultRootFrequency = 261.6256;
const systemParam = "system";
const rootFrequencyParam = "root";
const degreeParam = "degree";
const majorScaleSemitones = [0, 2, 4, 5, 7, 9, 11, 12];
const twelveToneFlatNames = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

let tuningSystems = [];
let selectedSystemId = "";
let selectedDegree = 0;
let audioContext = null;
let activeNodes = [];
let activeTimers = [];
let shareResetTimer = null;

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

async function loadTuningSystems({ syncUrl = false } = {}) {
  const candidates = ["../pkg/music21_rs_web.js"];
  let lastError = null;
  for (const candidate of candidates) {
    try {
      const module = await import(new URL(candidate, window.location.href).href);
      await module.default();
      if (typeof module.tuning_systems !== "function") {
        throw new Error("The current WASM package does not expose tuning_systems yet.");
      }
      const frequency = clampNumber(rootFrequency.value, 20, 2000, defaultRootFrequency);
      rootFrequency.value = frequency;
      tuningSystems = module.tuning_systems(frequency);
      if (!selectedSystemId || !tuningSystems.some((system) => system.id === selectedSystemId)) {
        selectedSystemId = tuningSystems[0]?.id ?? "";
      }
      clearError();
      render();
      if (syncUrl) syncShareUrl();
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
      resetShareButton();
      render();
      syncShareUrl();
    });
    systemsNode.appendChild(button);
  }
}

function renderSelectedSystem() {
  const system = selectedSystem();
  if (!system) return;
  selectedDegree = normalizeSelectedDegree(system, selectedDegree);
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
    label.textContent = displayDegreeLabel(system, degree);
    const frequency = document.createElement("small");
    frequency.textContent = `${formatFrequency(degree.frequency_hz)} Hz`;
    button.append(label, frequency);
    button.addEventListener("click", () => {
      selectedDegree = degree.degree;
      resetShareButton();
      renderScale(system);
      syncShareUrl();
      playDegree(degree);
    });
    scaleStrip.appendChild(button);
  }

  const playButton = document.createElement("button");
  playButton.type = "button";
  playButton.className = "degree-button scale-action";
  playButton.innerHTML = "<strong>Play scale</strong><small>all degrees</small>";
  playButton.addEventListener("click", () => playScale(system));
  scaleStrip.appendChild(playButton);

  const majorButton = document.createElement("button");
  majorButton.type = "button";
  majorButton.className = "degree-button scale-action";
  const isTwelveTone = Number(system.octave_size) === 12;
  const majorLabel = document.createElement("strong");
  majorLabel.textContent = isTwelveTone ? "Play major" : "Nearest major";
  const majorHint = document.createElement("small");
  majorHint.textContent = isTwelveTone ? "C D E F G A B" : "suggested degrees";
  const suggestedDegrees = suggestedMajorScaleDegrees(system).map((degree) => degree.degree);
  majorButton.title = isTwelveTone
    ? "Play the major scale"
    : `Suggested nearest major-scale degrees: ${suggestedDegrees.join(", ")}`;
  majorButton.append(majorLabel, majorHint);
  majorButton.addEventListener("click", () => playMajorScale(system));
  scaleStrip.appendChild(majorButton);
}

function renderDegrees(system) {
  degreesBody.replaceChildren();
  for (const degree of system.degrees) {
    const row = document.createElement("tr");
    for (const value of [
      degree.degree,
      displayDegreeLabel(system, degree),
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
      resetShareButton();
      renderScale(system);
      syncShareUrl();
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

function displayDegreeLabel(system, degree) {
  if (Number(system.octave_size) !== 12) return degree.label;
  const degreeNumber = Number(degree.degree);
  const noteName = twelveToneFlatNames[((degreeNumber % 12) + 12) % 12];
  const octave = 4 + Math.floor(degreeNumber / 12);
  return `${noteName}${octave}`;
}

function normalizeSelectedDegree(system, degreeValue) {
  const degree = Number.parseInt(String(degreeValue), 10);
  if (system.degrees.some((candidate) => candidate.degree === degree)) return degree;
  return 0;
}

function suggestedMajorScaleDegrees(system) {
  const octaveSize = Math.max(1, Number(system.octave_size) || 12);
  const degreeNumbers = majorScaleSemitones
    .map((semitone, index) => {
      if (Number(system.octave_size) === 12) return semitone;
      if (index === majorScaleSemitones.length - 1) return octaveSize;
      return Math.round((semitone * octaveSize) / 12);
    })
    .filter((degree, index, degrees) => degrees.indexOf(degree) === index);

  return degreeNumbers
    .map((degreeNumber) => system.degrees.find((degree) => degree.degree === degreeNumber))
    .filter(Boolean);
}

function getSharedSystemId() {
  const params = new URLSearchParams(window.location.search);
  return params.has(systemParam) ? (params.get(systemParam) ?? "") : null;
}

function getSharedRootFrequency() {
  const params = new URLSearchParams(window.location.search);
  if (!params.has(rootFrequencyParam)) return null;
  return clampNumber(params.get(rootFrequencyParam), 20, 2000, defaultRootFrequency);
}

function getSharedDegree() {
  const params = new URLSearchParams(window.location.search);
  if (!params.has(degreeParam)) return null;
  const degree = Number.parseInt(params.get(degreeParam) ?? "", 10);
  return Number.isFinite(degree) && degree >= 0 ? degree : 0;
}

function buildShareUrl() {
  const system = selectedSystem();
  const url = new URL(window.location.href);
  if (system) url.searchParams.set(systemParam, system.id);
  url.searchParams.set(
    rootFrequencyParam,
    String(clampNumber(rootFrequency.value, 20, 2000, defaultRootFrequency)),
  );
  if (selectedDegree > 0) {
    url.searchParams.set(degreeParam, String(selectedDegree));
  } else {
    url.searchParams.delete(degreeParam);
  }
  return url;
}

function syncShareUrl() {
  const url = buildShareUrl();
  window.history.replaceState(
    {
      degree: selectedDegree,
      rootFrequency: rootFrequency.value,
      system: selectedSystemId,
    },
    "",
    url.href,
  );
  return url.href;
}

function resetShareButton() {
  shareButton.textContent = "Share tuning";
  shareButton.classList.remove("copied");
}

function markShareCopied() {
  shareButton.textContent = "Copied";
  shareButton.classList.add("copied");
  if (shareResetTimer) clearTimeout(shareResetTimer);
  shareResetTimer = setTimeout(resetShareButton, 1600);
}

async function writeClipboard(value) {
  if (navigator.clipboard?.writeText && window.isSecureContext) {
    await navigator.clipboard.writeText(value);
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.setAttribute("readonly", "");
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("Copy failed");
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
  playDegrees(system, system.degrees);
}

async function playMajorScale(system) {
  if (!await ensureAudio()) return;
  playDegrees(system, suggestedMajorScaleDegrees(system));
}

function playDegrees(system, degrees) {
  stopPlayback();
  if (!degrees.length) return;
  const duration = noteDurationSeconds();
  const start = audioContext.currentTime + 0.04;
  degrees.forEach((degree, index) => {
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
  resetShareButton();
  loadTuningSystems({ syncUrl: true });
});
stopButton.addEventListener("click", stopPlayback);
rootFrequency.addEventListener("change", () => {
  resetShareButton();
  loadTuningSystems({ syncUrl: true });
});

shareButton.addEventListener("click", async () => {
  const href = syncShareUrl();
  try {
    await writeClipboard(href);
    clearError();
    markShareCopied();
  } catch {
    showError("The URL has been updated in your address bar.");
  }
});

window.addEventListener("popstate", () => {
  const sharedRootFrequency = getSharedRootFrequency();
  rootFrequency.value = sharedRootFrequency ?? defaultRootFrequency;
  selectedSystemId = getSharedSystemId() ?? "";
  selectedDegree = getSharedDegree() ?? 0;
  stopPlayback();
  resetShareButton();
  loadTuningSystems();
});

const sharedRootFrequency = getSharedRootFrequency();
if (sharedRootFrequency !== null) rootFrequency.value = sharedRootFrequency;
selectedSystemId = getSharedSystemId() ?? "";
selectedDegree = getSharedDegree() ?? 0;

loadTuningSystems();
