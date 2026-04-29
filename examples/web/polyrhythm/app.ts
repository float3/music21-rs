// @ts-nocheck
import "../help-tooltips.js";

const rhythmInput = document.querySelector("#rhythm-input");
const baseInput = document.querySelector("#base-input");
const tempoInput = document.querySelector("#tempo-input");
const cyclesInput = document.querySelector("#cycles-input");
const rootInput = document.querySelector("#root-input");
const form = document.querySelector("#form");
const share = document.querySelector("#share");
const playButton = document.querySelector("#play");
const stopButton = document.querySelector("#stop");
const randomPolyrhythm = document.querySelector("#random-polyrhythm");
const randomTrackMin = document.querySelector("#random-track-min");
const randomTrackMax = document.querySelector("#random-track-max");
const randomSubdivisionMin = document.querySelector("#random-subdivision-min");
const randomSubdivisionMax = document.querySelector("#random-subdivision-max");
const error = document.querySelector("#error");
const examples = document.querySelector("#examples");
const history = document.querySelector("#history");
const clearHistory = document.querySelector("#clear-history");
const facts = document.querySelector("#facts");
const chordName = document.querySelector("#chord-name");
const chordNotation = document.querySelector("#chord-notation");
const rhythmNotation = document.querySelector("#rhythm-notation");
const chordPitches = document.querySelector("#chord-pitches");
const chordRatios = document.querySelector("#chord-ratios");
const chordLink = document.querySelector("#chord-link");
const chordLinkTop = document.querySelector("#chord-link-top");
const browserLinkTop = document.querySelector("#browser-link-top");
const tuningLinkTop = document.querySelector("#tuning-link-top");
const docsLink = document.querySelector("#docs-link");
const cycleGrid = document.querySelector("#cycle-grid");
const eventsBody = document.querySelector("#events");

const pitchNames = ["C", "C#", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B"];
const voiceFrequencies = [176, 220, 264, 330, 396, 495, 594, 704];
const examplesList = ["2:3", "3:4", "4:5:6", "4:5:6:7", "5:6:7", "7:11:13"];
const historyStorageKey = "music21-rs.polyrhythmLab.history";
const maxHistoryItems = 24;
const denseCycleLimit = 192;
let analyzeChord = null;
let analyzePolyrhythm = null;
let initializedAnalyzer = false;
let currentAnalysis = null;
let audioContext = null;
let timers = [];
let shareResetTimer = null;
let polyrhythmHistory = loadPolyrhythmHistory();
let mutedTracks = new Set();

populateRootSelector();

const isLocalExample = ["127.0.0.1", "localhost"].includes(window.location.hostname)
  || window.location.protocol === "file:";
const chordBaseHref = isLocalExample ? "../chord/index.html" : "../chord/";
chordLinkTop.href = chordBaseHref;
browserLinkTop.href = isLocalExample ? "../chords/index.html" : "../chords/";
tuningLinkTop.href = isLocalExample ? "../tuning/index.html" : "../tuning/";
docsLink.href = "../docs/music21_rs/index.html";

for (const value of examplesList) {
  const button = document.createElement("button");
    button.type = "button";
    button.textContent = value;
    button.addEventListener("click", () => {
      rhythmInput.value = value;
      resetShareButton();
      update({ remember: true });
    });
    examples.appendChild(button);
  }

randomPolyrhythm.addEventListener("click", () => {
  rhythmInput.value = generateRandomPolyrhythm();
  resetShareButton();
  update({ remember: true });
});

function populateRootSelector() {
  rootInput.replaceChildren();
  for (const octave of [2, 3, 4, 5]) {
    for (const name of pitchNames) {
      const option = document.createElement("option");
      option.value = `${name}${octave}`;
      option.textContent = `${displayPitchName(name)}${octave}`;
      option.selected = option.value === "C4";
      rootInput.appendChild(option);
    }
  }
}

function generateRandomPolyrhythm() {
  const [trackMin, trackMax] = randomSettingRange(randomTrackMin, randomTrackMax, 1, 8, 2, 5);
  const [subdivisionMin, subdivisionMax] = randomSettingRange(
    randomSubdivisionMin,
    randomSubdivisionMax,
    1,
    64,
    2,
    13,
  );
  const availableSubdivisions = subdivisionMax - subdivisionMin + 1;
  const voiceCount = Math.min(
    randomInteger(trackMin, trackMax),
    availableSubdivisions,
  );
  const values = new Set();
  while (values.size < voiceCount) {
    values.add(randomInteger(subdivisionMin, subdivisionMax));
  }
  return [...values].sort((a, b) => a - b).join(":");
}

function randomSettingRange(minInput, maxInput, minAllowed, maxAllowed, fallbackMin, fallbackMax) {
  let min = clampInteger(minInput.value, minAllowed, maxAllowed, fallbackMin);
  let max = clampInteger(maxInput.value, minAllowed, maxAllowed, fallbackMax);
  if (min > max) [min, max] = [max, min];
  minInput.value = min;
  maxInput.value = max;
  return [min, max];
}

function randomInteger(min, max) {
  return min + Math.floor(Math.random() * (max - min + 1));
}

function getSharedRhythm() {
  const params = new URLSearchParams(window.location.search);
  return params.has("rhythm") ? (params.get("rhythm") ?? "") : null;
}

function buildShareUrl(value) {
  const url = new URL(window.location.href);
  url.searchParams.set("rhythm", normalizePolyrhythmInput(value));
  return url;
}

function syncShareUrl() {
  const url = buildShareUrl(rhythmInput.value);
  window.history.replaceState({ rhythm: rhythmInput.value }, "", url.href);
  return url.href;
}

function showError(message) {
  error.textContent = message;
  error.style.display = "block";
}

function clearError() {
  error.style.display = "none";
}

function clampInteger(value, min, max, fallback) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

function parseComponents(value) {
  const components = value
    .split(/[^0-9]+/)
    .map((part) => Number.parseInt(part, 10))
    .filter((part) => Number.isFinite(part));
  if (!components.length) {
    throw new Error("Enter at least one subdivision.");
  }
  if (components.some((part) => part < 1 || part > 64)) {
    throw new Error("Subdivisions must be between 1 and 64.");
  }
  return components;
}

function normalizePolyrhythmInput(value) {
  return parseComponents(value).join(":");
}

function loadPolyrhythmHistory() {
  try {
    const raw = window.localStorage?.getItem(historyStorageKey);
    if (!raw) return [];
    const values = JSON.parse(raw);
    if (!Array.isArray(values)) return [];

    const seen = new Set();
    return values
      .map((value) => {
        try {
          return typeof value === "string" ? normalizePolyrhythmInput(value) : "";
        } catch {
          return "";
        }
      })
      .filter((value) => {
        if (!value || seen.has(value)) return false;
        seen.add(value);
        return true;
      })
      .slice(0, maxHistoryItems);
  } catch {
    return [];
  }
}

function savePolyrhythmHistory() {
  try {
    window.localStorage?.setItem(historyStorageKey, JSON.stringify(polyrhythmHistory));
  } catch {
    // Browsers can deny localStorage; history still works for this page view.
  }
}

function renderPolyrhythmHistory() {
  history.replaceChildren();
  clearHistory.hidden = polyrhythmHistory.length === 0;
  for (const value of polyrhythmHistory) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = value;
    button.title = "Previous polyrhythm";
    button.addEventListener("click", () => {
      rhythmInput.value = value;
      resetShareButton();
      update({ remember: true });
    });
    history.appendChild(button);
  }
}

function clearPolyrhythmHistory() {
  polyrhythmHistory = [];
  savePolyrhythmHistory();
  renderPolyrhythmHistory();
}

function rememberPolyrhythm(value) {
  let normalized;
  try {
    normalized = normalizePolyrhythmInput(value);
  } catch {
    return;
  }

  polyrhythmHistory = [
    normalized,
    ...polyrhythmHistory.filter((historyValue) => historyValue !== normalized),
  ].slice(0, maxHistoryItems);
  savePolyrhythmHistory();
  renderPolyrhythmHistory();
}

function setPolyrhythmComponents(components, { remember = true } = {}) {
  if (!components.length) {
    showError("Keep at least one subdivision.");
    return;
  }
  rhythmInput.value = components.join(":");
  resetShareButton();
  pruneMutedTracks(components.length);
  update({ remember });
}

function updateTrackComponent(index, value) {
  const components = currentAnalysis?.components
    ? [...currentAnalysis.components]
    : parseComponents(rhythmInput.value);
  const nextValue = clampInteger(value, 1, 64, components[index] ?? 1);
  if (components[index] === nextValue) {
    if (currentAnalysis) renderCycle(currentAnalysis);
    return;
  }
  components[index] = nextValue;
  setPolyrhythmComponents(components);
}

function removeTrack(index) {
  const components = currentAnalysis?.components
    ? [...currentAnalysis.components]
    : parseComponents(rhythmInput.value);
  if (components.length <= 1) {
    showError("Keep at least one subdivision.");
    return;
  }
  components.splice(index, 1);
  mutedTracks = new Set(
    [...mutedTracks]
      .filter((trackIndex) => trackIndex !== index)
      .map((trackIndex) => (trackIndex > index ? trackIndex - 1 : trackIndex)),
  );
  setPolyrhythmComponents(components);
}

function toggleTrackMute(index) {
  if (mutedTracks.has(index)) {
    mutedTracks.delete(index);
  } else {
    mutedTracks.add(index);
  }
  if (currentAnalysis) renderCycle(currentAnalysis);
}

function pruneMutedTracks(trackCount) {
  mutedTracks = new Set([...mutedTracks].filter((index) => index < trackCount));
}

function displayPitchName(name) {
  return String(name).replaceAll("-", "b");
}

function buildAnalysis() {
  const components = parseComponents(rhythmInput.value);
  const base = clampInteger(baseInput.value, 1, 16, 4);
  const tempo = clampInteger(tempoInput.value, 20, 260, 120);
  const cycles = clampInteger(cyclesInput.value, 1, 16, 2);
  baseInput.value = base;
  tempoInput.value = tempo;
  cyclesInput.value = cycles;

  if (!analyzePolyrhythm) {
    throw new Error("Polyrhythm analysis is unavailable.");
  }

  const libraryAnalysis = analyzePolyrhythm(components, base, tempo, rootInput.value);
  const hitEvents = (libraryAnalysis.hit_events ?? []).map((event) => ({
    tick: event.tick,
    timeSeconds: event.time_seconds,
    triggers: event.triggers ?? [],
  }));
  const hitByTick = new Map(hitEvents.map((event) => [event.tick, event]));
  const ratioTones = libraryAnalysis.ratio_tones ?? [];
  const pitches = libraryAnalysis.pitches ?? [];
  return {
    components: libraryAnalysis.components ?? components,
    base: libraryAnalysis.base ?? base,
    tempo: libraryAnalysis.tempo ?? tempo,
    cycles,
    cycle: libraryAnalysis.cycle,
    tickDuration: libraryAnalysis.tick_duration,
    componentIntervals: libraryAnalysis.component_intervals ?? [],
    events: hitEvents,
    hitEvents,
    hitByTick,
    fullCycleView: libraryAnalysis.cycle <= denseCycleLimit,
    offsets: ratioTones.map((tone) => tone.offset),
    ratioTones,
    pitches,
    chordInput: libraryAnalysis.chord_input ?? pitches.join(" "),
  };
}

function renderFacts(data) {
  const rows = [
    ["Components", data.components.join(":")],
    ["Cycle ticks", data.cycle.toLocaleString()],
    ["Hit events", data.hitEvents.length.toLocaleString()],
    ["View", data.fullCycleView ? "full cycle" : "event-only"],
    ["Tick seconds", data.tickDuration.toFixed(4)],
    ["Chord tones", data.pitches.length],
  ];
  facts.replaceChildren();
  for (const [label, value] of rows) {
    const item = document.createElement("div");
    item.className = "fact";
    const labelNode = document.createElement("span");
    labelNode.textContent = label;
    const valueNode = document.createElement("strong");
    valueNode.textContent = value;
    item.append(labelNode, valueNode);
    facts.appendChild(item);
  }
}

function renderChips(node, values) {
  node.replaceChildren();
  for (const value of values) {
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.textContent = value;
    node.appendChild(chip);
  }
}

function renderNotation(pitchNames) {
  chordNotation.replaceChildren();
  const renderAbc = window.ABCJS?.renderAbc || window.abcjs?.renderAbc;
  if (!renderAbc) {
    const fallback = document.createElement("div");
    fallback.className = "notation-fallback";
    fallback.textContent = "Notation unavailable";
    chordNotation.appendChild(fallback);
    return;
  }

  try {
    renderAbc("chord-notation", buildAbc(pitchNames), {
      responsive: "resize",
      staffwidth: Math.max(280, chordNotation.clientWidth - 8),
      scale: 0.9,
    });
  } catch {
    const fallback = document.createElement("div");
    fallback.className = "notation-fallback";
    fallback.textContent = "Notation unavailable";
    chordNotation.appendChild(fallback);
  }
}

function renderRhythmNotation(data) {
  rhythmNotation.replaceChildren();
  const renderAbc = window.ABCJS?.renderAbc || window.abcjs?.renderAbc;
  if (!renderAbc) {
    const fallback = document.createElement("div");
    fallback.className = "notation-fallback";
    fallback.textContent = "Notation unavailable";
    rhythmNotation.appendChild(fallback);
    return;
  }

  try {
    renderAbc("rhythm-notation", buildRhythmAbc(data), {
      responsive: "resize",
      staffwidth: Math.max(360, rhythmNotation.clientWidth - 20),
      scale: 0.88,
    });
  } catch {
    const fallback = document.createElement("div");
    fallback.className = "notation-fallback";
    fallback.textContent = "Notation unavailable";
    rhythmNotation.appendChild(fallback);
  }
}

function eventHasTrigger(event, voiceIndex) {
  return Boolean(event.triggers?.[voiceIndex]);
}

function displayEventsForCycle(data) {
  if (!data.fullCycleView) return data.hitEvents;

  const events = [];
  for (let tick = 0; tick < data.cycle; tick += 1) {
    events.push(data.hitByTick.get(tick) ?? {
      tick,
      timeSeconds: tick * data.tickDuration,
      triggers: [],
    });
  }
  return events;
}

function buildRhythmAbc(data) {
  const lines = [
    "X:1",
    "L:1/4",
    `M:${data.base}/4`,
    "K:C clef=perc style=x",
  ];

  for (const [index, component] of data.components.entries()) {
    lines.push(`V:${index + 1} name="${component}" clef=perc style=x`);
    lines.push(`${buildRhythmVoiceAbc(component, data.base)} |]`);
  }

  return `${lines.join("\n")}\n`;
}

function buildRhythmVoiceAbc(component, base) {
  if (component === base) {
    return Array.from({ length: component }, () => "B").join(" ");
  }

  if (component === 1) {
    return `B${base}`;
  }

  if (component <= 9) {
    const notes = Array.from({ length: component }, () => "B").join(" ");
    return `(${component}:${base}:${component}${notes}`;
  }

  const duration = abcDuration(base, component);
  return Array.from({ length: component }, (_, index) => {
    const label = index === 0 ? `"^${component}:${base}"` : "";
    return `${label}B${duration}`;
  }).join(" ");
}

function abcDuration(numerator, denominator) {
  const divisor = durationGcd(Math.abs(numerator), Math.abs(denominator));
  const top = numerator / divisor;
  const bottom = denominator / divisor;

  if (bottom === 1) {
    return top === 1 ? "" : String(top);
  }

  return top === 1 ? `/${bottom}` : `${top}/${bottom}`;
}

function durationGcd(a, b) {
  while (b !== 0) {
    [a, b] = [b, a % b];
  }
  return Math.abs(a);
}

function buildAbc(pitchNames) {
  const notes = pitchNames.map(abcNote).filter(Boolean);
  const chord = notes.length ? `[${notes.join("")}]4` : "z4";
  return `X:1\nL:1/4\nM:4/4\nK:C clef=${chooseClef(pitchNames)}\n${chord} |]\n`;
}

function chooseClef(pitchNames) {
  const midiValues = pitchNames.map(pitchMidi).filter(Number.isFinite);
  if (!midiValues.length) return "treble";
  const average = midiValues.reduce((sum, value) => sum + value, 0) / midiValues.length;
  const lowest = Math.min(...midiValues);
  return average < 60 || lowest < 48 ? "bass" : "treble";
}

function pitchMidi(value) {
  const match = String(value).trim().match(/^([A-G])([#-]*)(-?\d+)?$/);
  if (!match) return Number.NaN;
  const natural = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 }[match[1]];
  const accidental = match[2]
    .split("")
    .reduce((sum, char) => sum + (char === "#" ? 1 : -1), 0);
  const octave = match[3] === undefined ? 4 : Number.parseInt(match[3], 10);
  if (!Number.isFinite(octave)) return Number.NaN;
  return (octave + 1) * 12 + natural + accidental;
}

function abcNote(value) {
  const match = String(value).trim().match(/^([A-G])([#-]*)(-?\d+)?$/);
  if (!match) return "";

  const step = match[1];
  const accidental = match[2]
    .split("")
    .map((char) => (char === "#" ? "^" : "_"))
    .join("");
  const octave = match[3] === undefined ? 4 : Number.parseInt(match[3], 10);
  if (!Number.isFinite(octave)) return "";

  if (octave >= 5) {
    return `${accidental}${step.toLowerCase()}${"'".repeat(octave - 5)}`;
  }
  return `${accidental}${step}${",".repeat(Math.max(0, 4 - octave))}`;
}

function chordPageUrl(chordInput) {
  const url = new URL(chordBaseHref, window.location.href);
  url.searchParams.set("chord", chordInput);
  return url.href;
}

function renderCycle(data) {
  cycleGrid.replaceChildren();
  pruneMutedTracks(data.components.length);
  cycleGrid.classList.toggle("compact", !data.fullCycleView);
  cycleGrid.style.gridTemplateRows = `repeat(${data.components.length}, auto)`;
  const displayEvents = displayEventsForCycle(data);
  const minColumnWidth = data.fullCycleView ? 10 : 12;
  for (const [voiceIndex, component] of data.components.entries()) {
    const isMuted = mutedTracks.has(voiceIndex);
    const row = document.createElement("div");
    row.className = `voice-row${isMuted ? " muted" : ""}`;
    row.title = isMuted ? "Click this track to unmute it" : "Click this track to mute it";
    row.addEventListener("click", () => toggleTrackMute(voiceIndex));

    const controls = document.createElement("div");
    controls.className = "voice-controls";
    controls.addEventListener("click", (event) => event.stopPropagation());

    const mute = document.createElement("button");
    mute.type = "button";
    mute.className = "voice-mute";
    mute.textContent = isMuted ? "Muted" : "On";
    mute.setAttribute("aria-pressed", String(isMuted));
    mute.title = isMuted ? "Unmute track" : "Mute track";
    mute.addEventListener("click", () => toggleTrackMute(voiceIndex));

    const number = document.createElement("input");
    number.className = "voice-number";
    number.type = "number";
    number.min = "1";
    number.max = "64";
    number.step = "1";
    number.value = component;
    number.setAttribute("aria-label", `Subdivision for track ${voiceIndex + 1}`);
    number.addEventListener("change", () => updateTrackComponent(voiceIndex, number.value));
    number.addEventListener("blur", () => updateTrackComponent(voiceIndex, number.value));
    number.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        updateTrackComponent(voiceIndex, number.value);
        number.blur();
      }
    });

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "voice-remove";
    remove.textContent = "x";
    remove.title = "Remove track";
    remove.setAttribute("aria-label", `Remove ${component} track`);
    remove.addEventListener("click", () => removeTrack(voiceIndex));
    controls.append(mute, number, remove);

    const ticks = document.createElement("div");
    ticks.className = `tick-row${data.fullCycleView ? "" : " compact"}`;
    ticks.style.gridTemplateColumns = `repeat(${displayEvents.length}, minmax(${minColumnWidth}px, 1fr))`;
    for (const event of displayEvents) {
      const isHit = eventHasTrigger(event, voiceIndex);
      const tick = document.createElement("div");
      tick.className = `tick${isHit ? " hit" : data.fullCycleView ? "" : " ghost"}`;
      tick.dataset.tick = event.tick;
      tick.title = `tick ${event.tick.toLocaleString()}`;
      ticks.appendChild(tick);
    }
    row.append(controls, ticks);
    cycleGrid.appendChild(row);
  }
}

function renderEvents(data) {
  eventsBody.replaceChildren();
  for (const event of data.hitEvents) {
    const tr = document.createElement("tr");
    const voices = event.triggers
      .map((trigger, index) => (trigger ? data.components[index] : null))
      .filter((value) => value !== null)
      .join(", ");
    for (const value of [event.tick.toLocaleString(), `${event.timeSeconds.toFixed(3)}s`, voices]) {
      const td = document.createElement("td");
      td.textContent = value;
      tr.appendChild(td);
    }
    eventsBody.appendChild(tr);
  }
}

async function loadAnalyzer() {
  if (initializedAnalyzer) return;
  initializedAnalyzer = true;
  const candidates = ["../pkg/music21_rs_web.js"];

  for (const candidate of candidates) {
    try {
      const module = await import(new URL(candidate, window.location.href).href);
      await module.default();
      analyzeChord = module.analyze_chord;
      analyzePolyrhythm = module.analyze_polyrhythm;
      return;
    } catch {
      // Try the next local/deployed package path.
    }
  }
}

async function renderChord(data) {
  await loadAnalyzer();
  let name = "Not available";
  if (analyzeChord) {
    try {
      name = analyzeChord(data.chordInput).pitched_common_name;
    } catch {
      name = "Not available";
    }
  }

  chordName.textContent = name;
  renderNotation(data.pitches);
  renderChips(chordPitches, data.pitches);
  renderChips(chordRatios, data.ratioTones.map((tone) => `${tone.component}`));
  chordLink.href = chordPageUrl(data.chordInput);
}

async function update({ remember = false, syncUrl = true } = {}) {
  stopPlayback();
  try {
    clearError();
    await loadAnalyzer();
    currentAnalysis = buildAnalysis();
    pruneMutedTracks(currentAnalysis.components.length);
    rhythmInput.value = currentAnalysis.components.join(":");
    if (syncUrl) syncShareUrl();
    renderFacts(currentAnalysis);
    renderRhythmNotation(currentAnalysis);
    renderCycle(currentAnalysis);
    renderEvents(currentAnalysis);
    await renderChord(currentAnalysis);
    if (remember) rememberPolyrhythm(rhythmInput.value);
  } catch (err) {
    currentAnalysis = null;
    showError(err instanceof Error ? err.message : String(err));
  }
}

function resetShareButton() {
  share.textContent = "Copy link";
  share.classList.remove("copied");
}

function markShareCopied() {
  share.textContent = "Copied";
  share.classList.add("copied");
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
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "-1000px";
  document.body.appendChild(textarea);
  textarea.select();
  try {
    if (!document.execCommand("copy")) {
      throw new Error("Copy command failed");
    }
  } finally {
    textarea.remove();
  }
}

function setActiveTick(tickIndex) {
  for (const tick of cycleGrid.querySelectorAll(".tick.active")) {
    tick.classList.remove("active");
  }
  for (const tick of cycleGrid.querySelectorAll(`[data-tick="${tickIndex}"]`)) {
    tick.classList.add("active");
  }
}

function playVoice(index, accent) {
  if (!audioContext) return;
  const now = audioContext.currentTime;
  const oscillator = audioContext.createOscillator();
  const gain = audioContext.createGain();
  oscillator.type = index === 0 ? "triangle" : "sine";
  oscillator.frequency.value = accent ? 880 : voiceFrequencies[index % voiceFrequencies.length];
  gain.gain.setValueAtTime(0.0001, now);
  gain.gain.exponentialRampToValueAtTime(accent ? 0.34 : 0.22, now + 0.008);
  gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.12);
  oscillator.connect(gain).connect(audioContext.destination);
  oscillator.start(now);
  oscillator.stop(now + 0.14);
}

async function startPlayback() {
  if (!currentAnalysis) await update();
  if (!currentAnalysis) return;
  stopPlayback();
  const AudioContextConstructor = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextConstructor) {
    showError("Audio is not available in this browser.");
    return;
  }
  audioContext ||= new AudioContextConstructor();
  if (audioContext.state === "suspended") {
    await audioContext.resume();
  }

  const analysis = currentAnalysis;
  const cycleMs = analysis.tickDuration * analysis.cycle * 1000;
  for (let cycleIndex = 0; cycleIndex < analysis.cycles; cycleIndex += 1) {
    const cycleOffsetMs = cycleIndex * cycleMs;
    for (const event of analysis.hitEvents) {
      const timer = window.setTimeout(() => {
        setActiveTick(event.tick);
        event.triggers.forEach((trigger, index) => {
          if (trigger && !mutedTracks.has(index)) playVoice(index, event.tick === 0);
        });
      }, cycleOffsetMs + event.timeSeconds * 1000);
      timers.push(timer);
    }
  }
  timers.push(window.setTimeout(stopPlayback, analysis.cycles * cycleMs + 160));
}

function stopPlayback() {
  for (const timer of timers) {
    window.clearTimeout(timer);
  }
  timers = [];
  setActiveTick(-1);
}

form.addEventListener("submit", (event) => {
  event.preventDefault();
  update({ remember: true });
});
share.addEventListener("click", async () => {
  let href;
  try {
    href = syncShareUrl();
  } catch (err) {
    showError(err instanceof Error ? err.message : String(err));
    return;
  }

  try {
    await writeClipboard(href);
    clearError();
    markShareCopied();
  } catch {
    showError("The URL has been updated in your address bar.");
  }
});
playButton.addEventListener("click", startPlayback);
stopButton.addEventListener("click", stopPlayback);
clearHistory.addEventListener("click", clearPolyrhythmHistory);

for (const element of [rhythmInput, baseInput, tempoInput, cyclesInput, rootInput]) {
  element.addEventListener("change", update);
}
rhythmInput.addEventListener("input", resetShareButton);
window.addEventListener("popstate", () => {
  const sharedRhythm = getSharedRhythm();
  if (sharedRhythm !== null) {
    rhythmInput.value = sharedRhythm;
    resetShareButton();
    update({ syncUrl: false });
  }
});

renderPolyrhythmHistory();
const sharedRhythm = getSharedRhythm();
if (sharedRhythm !== null) rhythmInput.value = sharedRhythm;
update({ syncUrl: false });
