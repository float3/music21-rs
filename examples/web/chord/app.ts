import "../help-tooltips.js";
import "../theme.js";
import init, {
  analyze_chord,
  analyze_chord_with_options,
  chord_resolution_abc,
  pitch_midi_number,
  twelve_tone_tuning_systems,
} from "../pkg/music21_rs_web.js";

type TuningFrequencyInfo = {
  id: string;
  name: string;
  frequency_hz: number;
  cents_from_equal_temperament: number;
};

type TuningSystemOption = {
  id: string;
  name: string;
  description: string;
};

type PitchInfo = {
  index: number;
  name: string;
  name_with_octave: string;
  midi: number;
  octave: number | null;
  pitch_space: number;
  pitch_class: number;
  alter: number;
  frequency_hz: number;
  tuning_frequencies?: TuningFrequencyInfo[];
};

type ResolutionChordInfo = {
  pitched_common_name: string;
  key_context: string;
  pitch_names: string[];
  pitch_classes: number[];
};

type RomanNumeralInfo = {
  figure: string;
  key_context: string;
};

type GuitarStringFingeringInfo = {
  string_number: number;
  string_name: string;
  open_pitch_space: number;
  open_pitch_class: number;
  fret: number | null;
  finger: number | null;
  pitch_space: number | null;
  pitch_class: number | null;
  pitch_name: string | null;
};

type GuitarFingeringInfo = {
  strings: GuitarStringFingeringInfo[];
  base_fret: number;
  fret_span: number;
  covered_pitch_spaces: number[];
  omitted_pitch_spaces: number[];
  covered_pitch_classes: number[];
  omitted_pitch_classes: number[];
};

type ChordAnalysis = {
  input: string;
  common_name: string;
  common_names: string[];
  pitched_common_name: string;
  pitched_common_names: string[];
  chord_symbol: string | null;
  chord_symbols: string[];
  pitch_classes: number[];
  root_pitch_name: string | null;
  bass_pitch_name: string | null;
  forte_class: string | null;
  normal_form: number[] | null;
  interval_class_vector: number[] | null;
  inversion: number | null;
  inversion_name: string | null;
  key_context: string | null;
  key_estimate: string | null;
  roman_numeral_context: RomanNumeralInfo | null;
  roman_numeral_estimate: RomanNumeralInfo | null;
  guitar_fingering: GuitarFingeringInfo | null;
  polyrhythm_input: string;
  resolution_chords: ResolutionChordInfo[];
  pitches: PitchInfo[];
  abc_notation: string;
};

type VexChordOptions = Record<string, string | number | boolean>;
type VexChordDefinition = {
  chord: Array<[number, number | "x"]>;
  position: number;
  bars: VexBarre[];
  barres: VexBarre[];
  tuning: string[];
};
type VexBarre = {
  fromString: number;
  toString: number;
  fret: number;
};
type VexChordBox = {
  draw: (chord: VexChordDefinition) => void;
};
type VexChordBoxConstructor = new (
  selector: string,
  options: VexChordOptions,
) => VexChordBox;
type VexRenderer = {
  draw?: (
    selector: string,
    chord: VexChordDefinition,
    options: VexChordOptions,
  ) => void;
  ChordBox?: VexChordBoxConstructor;
};

type AbcRenderer = (
  target: string,
  abc: string,
  options: { responsive: string; staffwidth: number; scale: number },
) => unknown;

type PlaybackMode = "arpeggio" | "block";
type FrequencySequenceItem = {
  frequencies: number[];
  duration: number;
  mode?: PlaybackMode;
  gapAfter?: number;
};
type AnalyzeOptions = {
  syncUrl?: boolean;
  remember?: boolean;
};

declare global {
  interface Window {
    vexchords?: unknown;
    VexChords?: unknown;
    Vexchords?: unknown;
    ABCJS?: { renderAbc?: AbcRenderer };
    abcjs?: { renderAbc?: AbcRenderer };
    webkitAudioContext?: typeof AudioContext;
  }
}

function mustQuery<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`Missing required element: ${selector}`);
  }
  return element;
}

const input = mustQuery<HTMLInputElement>("#chord-input");
const keyInput = mustQuery<HTMLInputElement>("#key-input");
const soundTuning = mustQuery<HTMLSelectElement>("#sound-tuning");
const guitarTuning = mustQuery<HTMLInputElement>("#guitar-tuning");
const estimateKey = mustQuery<HTMLButtonElement>("#estimate-key");
const form = mustQuery<HTMLFormElement>("#form");
const share = mustQuery<HTMLButtonElement>("#share");
const playChord = mustQuery<HTMLButtonElement>("#play-chord");
const midiStatus = mustQuery<HTMLElement>("#midi-status");
const historyOptions = mustQuery<HTMLDataListElement>("#chord-history");
const error = mustQuery<HTMLElement>("#error");
const browserLink = mustQuery<HTMLAnchorElement>("#browser-link");
const polyrhythmLink = mustQuery<HTMLAnchorElement>("#polyrhythm-link");
const tuningLink = mustQuery<HTMLAnchorElement>("#tuning-link");
const openPolyrhythm = mustQuery<HTMLAnchorElement>("#open-polyrhythm");
const facts = mustQuery<HTMLElement>("#facts");
const pitchedNames = mustQuery<HTMLElement>("#pitched-names");
const resolutionChords = mustQuery<HTMLElement>("#resolution-chords");
const guitarFingering = mustQuery<HTMLElement>("#guitar-fingering");
const pitches = mustQuery<HTMLTableSectionElement>("#pitches");
const keyboard = mustQuery<HTMLElement>("#keyboard");
const notation = mustQuery<HTMLElement>("#notation");
const randomChord = mustQuery<HTMLButtonElement>("#random-chord");
const randomMinNotes = mustQuery<HTMLInputElement>("#random-min-notes");
const randomMaxNotes = mustQuery<HTMLInputElement>("#random-max-notes");
const examples = mustQuery<HTMLElement>("#examples");
const history = mustQuery<HTMLElement>("#history");
const clearHistory = mustQuery<HTMLButtonElement>("#clear-history");
const pcNames = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const pcAltNames = ["B#", "Db", "D", "D#", "Fb", "E#", "Gb", "G", "G#", "Bbb", "A#", "Cb"];
const inputPitchNames = ["C", "C#", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B"];
const blackKeys = new Set([1, 3, 6, 8, 10]);
const keyboardStartMidi = 60;
const keyboardKeyCount = 24;
const chordParam = "chord";
const keyParam = "key";
const soundTuningParam = "soundTuning";
const guitarTuningParam = "guitarTuning";
const defaultSoundTuningId = "EqualTemperament";
const defaultGuitarTuning = "E2 A2 D3 G3 B3 E4";
const vexChordsUrl = "https://cdn.jsdelivr.net/npm/vexchords@1.2.0/dist/vexchords.dev.js";
const historyStorageKey = "music21-rs.chordInspector.history";
const maxHistoryItems = 24;
const randomNoteLimitMin = 1;
const randomNoteLimitMax = 12;
const randomNoteDefaultMin = 3;
const randomNoteDefaultMax = 6;
let shareResetTimer: number | null = null;
let currentAnalysis: ChordAnalysis | null = null;
let guitarRenderToken = 0;
let vexChordsPromise: Promise<VexRenderer | null> | null = null;
let audioContext: AudioContext | null = null;
let activeChordNodes: OscillatorNode[] = [];
let nextChordPlaybackMode: PlaybackMode = "arpeggio";
let midiAccess: MIDIAccess | null = null;
let heldMidiNotes = new Map<number, number>();
let chordHistory = loadChordHistory();
let chordHistoryLabels = new Map<string, string>();

const isFileExample = window.location.protocol === "file:";

if (isFileExample) {
  browserLink.href = "../chords/index.html";
  polyrhythmLink.href = "../polyrhythm/index.html";
  tuningLink.href = "../tuning/index.html";
} else if (!window.location.pathname.includes("/chord/")) {
  browserLink.href = "./chords/";
  polyrhythmLink.href = "./polyrhythm/";
  tuningLink.href = "./tuning/";
}

function text(value: unknown): string {
  if (value === null || value === undefined) return "Not available";
  if (Array.isArray(value)) return value.length ? value.join(", ") : "Not available";
  return String(value);
}

function romanText(value: RomanNumeralInfo | null | undefined): string {
  if (!value?.figure) return "Not available";
  return value.key_context ? `${value.figure} in ${value.key_context}` : value.figure;
}

function renderChips(node: HTMLElement, values: string[] | null | undefined): void {
  node.replaceChildren();
  const list = values && values.length ? values : ["Not available"];
  for (const value of list) {
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.textContent = value;
    node.appendChild(chip);
  }
}

function renderFacts(data: ChordAnalysis): void {
  const helpText: Record<string, string> = {
    "Forte class": "A catalog label for the chord's pitch-class set, grouping transpositionally or inversionally related sets.",
    "Normal form": "The chord's pitch classes rotated into their most compact ascending order.",
    "Interval-class vector": "A six-number count of the unordered interval classes contained inside the pitch-class set.",
  };
  const rows: Array<[string, unknown]> = [
    ["Chord symbol", data.chord_symbol],
    ["Common name", data.common_name],
    ["Root", data.root_pitch_name],
    ["Bass", data.bass_pitch_name],
    ["Key context", data.key_context],
    ["Roman numeral (context)", romanText(data.roman_numeral_context)],
    ["Key estimate", data.key_estimate],
    ["Roman numeral (estimate)", romanText(data.roman_numeral_estimate)],
    ["Inversion", data.inversion_name ?? data.inversion],
    ["Forte class", data.forte_class],
    ["Normal form", data.normal_form],
    ["Interval-class vector", data.interval_class_vector],
  ];
  facts.replaceChildren();
  for (const [label, value] of rows) {
    const item = document.createElement("div");
    item.className = "fact";
    const labelNode = document.createElement("span");
    labelNode.textContent = label;
    if (helpText[label]) {
      const help = document.createElement("button");
      help.type = "button";
      help.className = "help";
      help.textContent = "?";
      help.dataset.help = helpText[label];
      help.setAttribute("aria-label", `${label}: ${helpText[label]}`);
      labelNode.appendChild(help);
    }
    const valueNode = document.createElement("strong");
    valueNode.textContent = text(value);
    item.append(labelNode, valueNode);
    facts.appendChild(item);
  }
}

function renderResolutions(data: ChordAnalysis): void {
  resolutionChords.replaceChildren();
  const values = data.resolution_chords || [];
  if (!values.length) {
    const empty = document.createElement("span");
    empty.className = "resolution-chip unavailable";
    empty.textContent = "Not available";
    resolutionChords.appendChild(empty);
    return;
  }

  for (const value of values) {
    const item = document.createElement("div");
    item.className = "resolution-item";
    const button = document.createElement("button");
    button.type = "button";
    button.className = "resolution-chip";
    const name = document.createElement("strong");
    name.textContent = value.pitched_common_name;
    const context = document.createElement("span");
    context.textContent = value.key_context;
    const pitchesText = document.createElement("small");
    pitchesText.textContent = text(value.pitch_names);
    button.append(name, context, pitchesText);
    item.addEventListener("pointerenter", () => showResolutionMotion(value));
    item.addEventListener("pointerleave", clearResolutionMotion);
    item.addEventListener("focusin", () => showResolutionMotion(value));
    item.addEventListener("focusout", (event) => {
      if (!item.contains(event.relatedTarget as Node | null)) clearResolutionMotion();
    });
    button.addEventListener("click", async () => {
      clearResolutionMotion();
      await openResolution(value);
    });

    const preview = document.createElement("button");
    preview.type = "button";
    preview.className = "resolution-play";
    preview.textContent = "Play";
    preview.setAttribute(
      "aria-label",
      `Play current chord then ${value.pitched_common_name}`,
    );
    preview.addEventListener("click", async (event) => {
      event.stopPropagation();
      await playResolutionPreview(value);
    });

    item.append(button, preview);
    resolutionChords.appendChild(item);
  }
}

function formatHz(value: number): string {
  return Number.isFinite(value) ? `${value.toFixed(3)} Hz` : "Not available";
}

function formatCents(value: number): string {
  if (!Number.isFinite(value)) return "";
  const rounded = Math.abs(value) < 0.005 ? 0 : value;
  return `${rounded > 0 ? "+" : ""}${rounded.toFixed(2)} cents`;
}

function renderFrequencyCell(pitch: PitchInfo): HTMLTableCellElement {
  const td = document.createElement("td");
  td.className = "frequency-cell";
  const list = document.createElement("div");
  list.className = "tuning-list";
  const tuningFrequencies =
    pitch.tuning_frequencies && pitch.tuning_frequencies.length
      ? pitch.tuning_frequencies
      : [
          {
            id: defaultSoundTuningId,
            name: "Equal temperament",
            frequency_hz: pitch.frequency_hz,
            cents_from_equal_temperament: 0,
          },
        ];

  for (const tuning of tuningFrequencies) {
    const row = document.createElement("div");
    row.className =
      tuning.id === currentSoundTuningId()
        ? "tuning-row selected"
        : "tuning-row";
    const name = document.createElement("span");
    name.className = "tuning-name";
    name.textContent = tuning.name;
    const hz = document.createElement("span");
    hz.className = "tuning-hz";
    hz.textContent = formatHz(tuning.frequency_hz);
    row.append(name, hz);

    if (Math.abs(tuning.cents_from_equal_temperament) >= 0.005) {
      const cents = document.createElement("span");
      cents.className = "tuning-cents";
      cents.textContent = formatCents(tuning.cents_from_equal_temperament);
      row.appendChild(cents);
    }

    list.appendChild(row);
  }

  td.appendChild(list);
  return td;
}

function renderPitches(data: ChordAnalysis): void {
  pitches.replaceChildren();
  for (const pitch of data.pitches) {
    const tr = document.createElement("tr");
    const cells = [
      pitch.index + 1,
      displayPitchName(pitch.name_with_octave || pitch.name),
      pitch.midi,
      pitch.octave ?? "None",
      pitch.pitch_space.toFixed(3),
    ];
    for (const cell of cells) {
      const td = document.createElement("td");
      td.textContent = String(cell);
      tr.appendChild(td);
    }
    tr.appendChild(renderFrequencyCell(pitch));
    for (const cell of [pitch.pitch_class, pitch.alter.toFixed(3)]) {
      const td = document.createElement("td");
      td.textContent = String(cell);
      tr.appendChild(td);
    }
    pitches.appendChild(tr);
  }
}

function displayPitchName(value: unknown): string {
  const match = String(value ?? "").match(/^([A-G])([#-]*)(-?\d+)?$/);
  if (!match) return String(value ?? "");
  return `${match[1]}${match[2].replaceAll("-", "b")}${match[3] ?? ""}`;
}

function cssVar(name: string, fallback: string): string {
  return (
    getComputedStyle(document.documentElement).getPropertyValue(name).trim() ||
    fallback
  );
}

function renderKeyboard(pitchData: PitchInfo[]): void {
  const active = new Set(pitchData.map((pitch) => pitch.midi));
  keyboard.replaceChildren();
  for (let index = 0; index < keyboardKeyCount; index += 1) {
    const midi = keyboardStartMidi + index;
    const pitchClass = ((midi % 12) + 12) % 12;
    const octaveNumber = Math.floor(midi / 12) - 1;
    const name = pcNames[pitchClass];
    const pitchName = keyboardPitchName(midi);
    const isActive = active.has(midi);
    const key = document.createElement("button");
    key.type = "button";
    key.className = `key${blackKeys.has(pitchClass) ? " black" : ""}${isActive ? " active" : ""}`;
    const primary = document.createElement("span");
    primary.className = "key-name";
    primary.textContent = name;
    const alternate = document.createElement("span");
    alternate.className = "key-alt";
    alternate.textContent = pcAltNames[pitchClass] === name ? "" : pcAltNames[pitchClass];
    const octave = document.createElement("span");
    octave.className = "key-octave";
    octave.textContent = String(octaveNumber);
    key.append(primary, alternate, octave);
    key.title = isActive ? `Remove ${displayPitchName(pitchName)}` : `Add ${displayPitchName(pitchName)}`;
    key.setAttribute(
      "aria-label",
      isActive ? `Remove ${displayPitchName(pitchName)}` : `Add ${displayPitchName(pitchName)}`,
    );
    key.addEventListener("click", () => {
      toggleKeyboardPitch(midi);
    });
    keyboard.appendChild(key);
  }
}

async function renderGuitarFingering(fingering: GuitarFingeringInfo | null): Promise<void> {
  const renderToken = ++guitarRenderToken;
  guitarFingering.replaceChildren();
  if (!fingering) {
    const empty = document.createElement("div");
    empty.className = "guitar-empty";
    empty.textContent = "Not available";
    guitarFingering.appendChild(empty);
    return;
  }

  const chart = document.createElement("div");
  chart.id = "guitar-fingering-chart";
  chart.className = "guitar-chart";
  guitarFingering.appendChild(chart);

  const vex = await loadVexChordsRenderer();
  if (renderToken !== guitarRenderToken) return;
  if (vex) {
    try {
      drawVexChord(vex, `#${chart.id}`, fingering);
    } catch (err) {
      renderGuitarUnavailable(chart, err);
    }
  } else {
    renderGuitarUnavailable(chart);
  }

  const covered = document.createElement("div");
  covered.className = "guitar-note-list";
  for (const pitchClass of fingering.covered_pitch_classes || []) {
    const note = document.createElement("span");
    note.className = "guitar-note";
    note.textContent = displayPitchClassName(pitchClass);
    covered.appendChild(note);
  }
  guitarFingering.appendChild(covered);

  if (fingering.omitted_pitch_classes?.length) {
    const omitted = document.createElement("div");
    omitted.className = "guitar-note-list guitar-omitted";
    const label = document.createElement("span");
    label.textContent = "Omitted";
    omitted.appendChild(label);
    for (const pitchClass of fingering.omitted_pitch_classes) {
      const note = document.createElement("span");
      note.className = "guitar-note";
      note.textContent = displayPitchClassName(pitchClass);
      omitted.appendChild(note);
    }
    guitarFingering.appendChild(omitted);
  }
}

function renderGuitarUnavailable(node: HTMLElement, reason: unknown = null): void {
  if (reason) console.error("VexChords render failed", reason);
  node.replaceChildren();
  const empty = document.createElement("div");
  empty.className = "guitar-empty";
  empty.textContent = reason
    ? `Guitar chart failed: ${reason instanceof Error ? reason.message : String(reason)}`
    : "VexChords unavailable";
  node.appendChild(empty);
}

function vexChordsRenderer(): unknown {
  return window.vexchords || window.VexChords || window.Vexchords || null;
}

function loadVexChordsRenderer(): Promise<VexRenderer | null> {
  const globalRenderer = normalizeVexChordsRenderer(vexChordsRenderer());
  if (globalRenderer) return Promise.resolve(globalRenderer);
  vexChordsPromise ||= loadScript(vexChordsUrl)
    .then(() => normalizeVexChordsRenderer(vexChordsRenderer()))
    .catch((err) => {
      console.error("VexChords load failed", err);
      return null;
    });
  return vexChordsPromise;
}

function loadScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[src="${src}"]`);
    if (existing) {
      existing.addEventListener("load", () => resolve(), { once: true });
      existing.addEventListener("error", () => reject(new Error(`Failed to load ${src}`)), {
        once: true,
      });
      return;
    }

    const script = document.createElement("script");
    script.src = src;
    script.async = true;
    script.addEventListener("load", () => resolve(), { once: true });
    script.addEventListener("error", () => reject(new Error(`Failed to load ${src}`)), {
      once: true,
    });
    document.head.appendChild(script);
  });
}

function normalizeVexChordsRenderer(module: unknown): VexRenderer | null {
  const moduleRecord = module as Record<string, unknown> | null | undefined;
  const defaultRecord = moduleRecord?.default as Record<string, unknown> | null | undefined;
  for (const candidate of [
    module,
    moduleRecord?.default,
    moduleRecord?.vexchords,
    defaultRecord?.default,
  ]) {
    if (!candidate) continue;
    const candidateRecord = candidate as Record<string, unknown>;
    if (typeof candidateRecord.draw === "function") {
      return {
        draw: candidateRecord.draw.bind(candidate) as VexRenderer["draw"],
      };
    }
    if (typeof candidateRecord.ChordBox === "function") {
      return { ChordBox: candidateRecord.ChordBox as VexChordBoxConstructor };
    }
    if (typeof candidate === "function") {
      return { ChordBox: candidate as VexChordBoxConstructor };
    }
  }
  return null;
}

function drawVexChord(
  vex: VexRenderer,
  selector: string,
  fingering: GuitarFingeringInfo,
): void {
  const chord = vexChordForFingering(fingering);
  const options = vexChordOptions(fingering);
  if (typeof vex.draw === "function") {
    vex.draw(selector, chord, options);
    return;
  }
  if (typeof vex.ChordBox === "function") {
    const chordBox = new vex.ChordBox(selector, options);
    chordBox.draw(chord);
    return;
  }
  throw new Error("VexChords renderer has no draw API");
}

function vexChordOptions(fingering: GuitarFingeringInfo): VexChordOptions {
  const ink = cssVar("--ink", "#151515");
  const panel = cssVar("--panel", "#ffffff");
  const accent = cssVar("--accent", "#0f766e");
  const muted = cssVar("--muted", "#61646b");
  return {
    width: 190,
    height: 220,
    numStrings: Math.max(1, (fingering.strings || []).length),
    numFrets: 5,
    showTuning: true,
    defaultColor: ink,
    bgColor: panel,
    strokeColor: accent,
    textColor: ink,
    stringColor: muted,
    fretColor: muted,
    labelColor: muted,
  };
}

function vexChordForFingering(fingering: GuitarFingeringInfo): VexChordDefinition {
  const position = fingering.base_fret > 1 ? fingering.base_fret : 0;
  const barres = vexBarresForFingering(fingering, position);
  return {
    chord: (fingering.strings || []).map((string) => [
      string.string_number,
      vexFretValue(string.fret, position),
    ]),
    position,
    bars: barres,
    barres,
    tuning: (fingering.strings || []).map((string) => vexTuningLabel(string.string_name)),
  };
}

function vexTuningLabel(value: string | null | undefined): string {
  return String(value ?? "").replace(/-?\d+$/, "");
}

function vexFretValue(fret: number | null | undefined, position: number): number | "x" {
  if (fret === null || fret === undefined) return "x";
  if (fret === 0 || position <= 1) return fret;
  return fret - position + 1;
}

function vexBarresForFingering(
  fingering: GuitarFingeringInfo,
  position: number,
): VexBarre[] {
  const groups = new Map<string, number[]>();
  for (const string of fingering.strings || []) {
    const fretValue = string.fret;
    const finger = string.finger;
    if (
      fretValue === null ||
      finger === null ||
      !Number.isFinite(fretValue) ||
      fretValue <= 0 ||
      !Number.isFinite(finger)
    ) {
      continue;
    }
    const fret = vexFretValue(fretValue, position);
    const key = `${finger}:${fret}`;
    const strings = groups.get(key) || [];
    strings.push(string.string_number);
    groups.set(key, strings);
  }

  const barres: VexBarre[] = [];
  for (const [key, strings] of groups) {
    if (strings.length < 2) continue;
    const sorted = strings.sort((left, right) => right - left);
    const isContiguous = sorted.every((string, index) => index === 0 || sorted[index - 1] - string === 1);
    if (!isContiguous) continue;
    const [, fret] = key.split(":");
    barres.push({
      fromString: sorted[0],
      toString: sorted[sorted.length - 1],
      fret: Number(fret),
    });
  }
  return barres;
}

function displayPitchClassName(pitchClass: number): string {
  return pcNames[((pitchClass % 12) + 12) % 12];
}

function keyboardPitchName(midi: number): string {
  const pitchClass = ((midi % 12) + 12) % 12;
  const octave = Math.floor(midi / 12) - 1;
  return `${inputPitchNames[pitchClass]}${octave}`;
}

function toggleKeyboardPitch(midi: number): void {
  if (isMidiChordInput(input.value)) {
    const tokens = midiInputTokens(input.value);
    const hasPitch = tokens.some((token) => midiNumberForInputToken(token) === midi);
    const nextTokens = hasPitch
      ? tokens.filter((token) => midiNumberForInputToken(token) !== midi)
      : [...tokens, String(midi)];
    input.value = nextTokens.length ? `midi: ${nextTokens.join(" ")}` : "";
  } else {
    const tokens = chordInputTokens(input.value);
    const hasPitch = tokens.some((token) => midiNumberForInputToken(token) === midi);
    if (hasPitch) {
      input.value = tokens
        .filter((token) => midiNumberForInputToken(token) !== midi)
        .join(" ");
    } else {
      const pitchName = keyboardPitchName(midi);
      input.value = tokens.length ? `${tokens.join(" ")} ${pitchName}` : pitchName;
    }
  }
  resetShareButton();
  analyze({ remember: Boolean(normalizeChordInput(input.value)) });
}

function isMidiChordInput(value: string): boolean {
  return /^midi(?::|\s+)/i.test(normalizeChordInput(value));
}

function midiInputTokens(value: string): string[] {
  const normalized = normalizeChordInput(value);
  const body = normalized.toLowerCase().startsWith("midi:")
    ? normalized.slice(normalized.indexOf(":") + 1)
    : normalized.replace(/^midi\s+/i, "");
  return body.split(/[\s,]+/).filter((token) => /^-?\d+$/.test(token));
}

function midiNumberForInputToken(value: string): number | null {
  try {
    return pitch_midi_number(value);
  } catch {
    return null;
  }
}

function chordInputTokens(value: string): string[] {
  return normalizeChordInput(value)
    .split(/\s+/)
    .filter(Boolean);
}

function addPitchToChord(pitchName: string): void {
  const current = normalizeChordInput(input.value);
  input.value = current ? `${current} ${pitchName}` : pitchName;
  resetShareButton();
  analyze({ remember: true });
}

function renderNotation(
  data: ChordAnalysis,
  resolutionAnalysis: ChordAnalysis | null = null,
): void {
  notation.replaceChildren();
  const abc = resolutionAnalysis
    ? chord_resolution_abc(data.input, resolutionAnalysis.input)
    : data.abc_notation;
  const renderAbc = window.ABCJS?.renderAbc || window.abcjs?.renderAbc;
  if (!renderAbc) {
    const fallback = document.createElement("div");
    fallback.className = "notation-fallback";
    fallback.textContent = "Notation unavailable";
    notation.appendChild(fallback);
    return;
  }

  try {
    renderAbc("notation", abc, {
      responsive: "resize",
      staffwidth: Math.max(360, notation.clientWidth - 20),
      scale: 0.95,
    });
  } catch {
    const fallback = document.createElement("div");
    fallback.className = "notation-fallback";
    fallback.textContent = "Notation unavailable";
    notation.appendChild(fallback);
  }
}

function clearResolutionMotion() {
  if (currentAnalysis) renderNotation(currentAnalysis);
}

function showResolutionMotion(resolution: ResolutionChordInfo): void {
  if (!currentAnalysis) return;
  let resolutionAnalysis;
  try {
    resolutionAnalysis = analyzeWithCurrentOptions(chordValueForResolution(resolution));
  } catch {
    clearResolutionMotion();
    return;
  }

  renderNotation(currentAnalysis, resolutionAnalysis);
}

function render(data: ChordAnalysis): void {
  currentAnalysis = data;
  error.style.display = "none";
  renderFacts(data);
  renderChips(pitchedNames, data.pitched_common_names);
  renderResolutions(data);
  void renderGuitarFingering(data.guitar_fingering);
  renderPitches(data);
  renderKeyboard(data.pitches);
  renderNotation(data);
  renderPolyrhythmLink(data);
}

document.addEventListener("music21-theme-change", () => {
  if (!currentAnalysis) return;
  void renderGuitarFingering(currentAnalysis.guitar_fingering);
});

function renderPolyrhythmLink(data: ChordAnalysis): void {
  const rhythm = data.polyrhythm_input || "1";
  const url = new URL(
    isFileExample ? "../polyrhythm/index.html" : "../polyrhythm/",
    window.location.href,
  );
  url.searchParams.set("rhythm", rhythm);
  openPolyrhythm.href = url.href;
}

function getSharedChord(): string | null {
  const params = new URLSearchParams(window.location.search);
  return params.has(chordParam) ? (params.get(chordParam) ?? "") : null;
}

function getSharedKey(): string | null {
  const params = new URLSearchParams(window.location.search);
  return params.has(keyParam) ? (params.get(keyParam) ?? "") : null;
}

function getSharedSoundTuning(): string | null {
  const params = new URLSearchParams(window.location.search);
  return params.has(soundTuningParam)
    ? (params.get(soundTuningParam) ?? "")
    : null;
}

function getSharedGuitarTuning(): string | null {
  const params = new URLSearchParams(window.location.search);
  return params.has(guitarTuningParam)
    ? (params.get(guitarTuningParam) ?? "")
    : null;
}

function buildShareUrl(
  value: string,
  keyValue = keyInput.value,
  soundTuningValue = currentSoundTuningId(),
  guitarTuningValue = guitarTuning.value,
): URL {
  const url = new URL(window.location.href);
  url.searchParams.set(chordParam, value);
  const key = normalizeKeyInput(keyValue);
  if (key) {
    url.searchParams.set(keyParam, key);
  } else {
    url.searchParams.delete(keyParam);
  }
  if (soundTuningValue && soundTuningValue !== defaultSoundTuningId) {
    url.searchParams.set(soundTuningParam, soundTuningValue);
  } else {
    url.searchParams.delete(soundTuningParam);
  }
  const normalizedGuitarTuning = normalizeGuitarTuning(guitarTuningValue);
  if (
    normalizedGuitarTuning &&
    normalizedGuitarTuning !== defaultGuitarTuning
  ) {
    url.searchParams.set(guitarTuningParam, normalizedGuitarTuning);
  } else {
    url.searchParams.delete(guitarTuningParam);
  }
  return url;
}

function syncShareUrl(): string {
  const url = buildShareUrl(input.value);
  window.history.replaceState(
    {
      chord: input.value,
      guitarTuning: guitarTuning.value,
      key: keyInput.value,
      soundTuning: soundTuning.value,
    },
    "",
    url.href,
  );
  return url.href;
}

function normalizeChordInput(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

function normalizeKeyInput(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

function normalizeGuitarTuning(value: string): string {
  return value.trim().replace(/[,\s]+/g, " ");
}

function currentKeyContext(): string {
  return normalizeKeyInput(keyInput.value);
}

function currentGuitarTuning(): string {
  return normalizeGuitarTuning(guitarTuning.value);
}

function currentSoundTuningId(): string {
  return soundTuning.value || defaultSoundTuningId;
}

function populateSoundTuningOptions(): void {
  let systems: TuningSystemOption[] = [];
  try {
    systems = twelve_tone_tuning_systems() as TuningSystemOption[];
  } catch (err) {
    console.error("Twelve-tone tuning systems unavailable", err);
  }

  if (!systems.length) {
    systems = [
      {
        id: defaultSoundTuningId,
        name: "Equal temperament",
        description: "Twelve equal divisions of the octave.",
      },
    ];
  }

  soundTuning.replaceChildren();
  for (const system of systems) {
    const option = document.createElement("option");
    option.value = system.id;
    option.textContent = system.name;
    option.title = system.description;
    soundTuning.appendChild(option);
  }

  setSoundTuning(getSharedSoundTuning());
}

function setSoundTuning(value: string | null): void {
  const options = Array.from(soundTuning.options, (option) => option.value);
  soundTuning.value =
    value && options.includes(value) ? value : defaultSoundTuningId;
  if (!soundTuning.value && soundTuning.options.length) {
    soundTuning.selectedIndex = 0;
  }
}

function setGuitarTuning(value: string | null): void {
  guitarTuning.value = normalizeGuitarTuning(value || defaultGuitarTuning);
}

function analyzeWithCurrentOptions(chordValue: string): ChordAnalysis {
  return analyze_chord_with_options(
    chordValue,
    currentKeyContext(),
    currentGuitarTuning(),
  ) as ChordAnalysis;
}

function loadChordHistory(): string[] {
  try {
    const raw = window.localStorage?.getItem(historyStorageKey);
    if (!raw) return [];
    const values = JSON.parse(raw);
    if (!Array.isArray(values)) return [];

    const seen = new Set<string>();
    return values
      .map((value) => (typeof value === "string" ? normalizeChordInput(value) : ""))
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

function saveChordHistory(): void {
  try {
    window.localStorage?.setItem(historyStorageKey, JSON.stringify(chordHistory));
  } catch {
    // Browsers can deny localStorage; history still works for this page view.
  }
}

function renderChordHistory(): void {
  historyOptions.replaceChildren();
  history.replaceChildren();
  clearHistory.hidden = chordHistory.length === 0;

  for (const value of chordHistory) {
    const label = chordHistoryLabels.get(value) || historyLabelForChord(value);
    const option = document.createElement("option");
    option.value = value;
    historyOptions.appendChild(option);

    const button = document.createElement("button");
    button.type = "button";
    button.textContent = label;
    button.title = "Previous chord";
    button.addEventListener("click", () => {
      input.value = value;
      resetShareButton();
      analyze({ remember: true });
    });
    history.appendChild(button);
  }
}

function clearChordHistory(): void {
  chordHistory = [];
  chordHistoryLabels.clear();
  saveChordHistory();
  renderChordHistory();
}

function historyLabelForChord(value: string): string {
  try {
    const data = analyze_chord(value) as ChordAnalysis;
    const label = data.pitched_common_names?.[0] || data.pitched_common_name || value;
    chordHistoryLabels.set(value, label);
    return label;
  } catch {
    return value;
  }
}

function rememberChord(value: string): void {
  const normalized = normalizeChordInput(value);
  if (!normalized) return;
  historyLabelForChord(normalized);

  chordHistory = [
    normalized,
    ...chordHistory.filter((historyValue) => historyValue !== normalized),
  ].slice(0, maxHistoryItems);
  saveChordHistory();
  renderChordHistory();
}

function resetShareButton(): void {
  share.textContent = "Copy link";
  share.classList.remove("copied");
}

function markShareCopied(): void {
  share.textContent = "Copied";
  share.classList.add("copied");
  if (shareResetTimer) clearTimeout(shareResetTimer);
  shareResetTimer = setTimeout(resetShareButton, 1600);
}

async function writeClipboard(value: string): Promise<void> {
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

async function ensureAudio(): Promise<boolean> {
  const AudioContextConstructor = window.AudioContext || window.webkitAudioContext;
  if (!AudioContextConstructor) {
    error.textContent = "Audio is not available in this browser.";
    error.style.display = "block";
    return false;
  }

  try {
    audioContext ||= new AudioContextConstructor();
    if (audioContext.state === "suspended") {
      await audioContext.resume();
    }
  } catch {
    error.textContent = "Audio playback needs a browser click first.";
    error.style.display = "block";
    return false;
  }
  return true;
}

function stopChordPlayback(): void {
  for (const node of activeChordNodes) {
    try {
      node.stop();
    } catch {
      // Already stopped.
    }
  }
  activeChordNodes = [];
}

function chordValueForResolution(resolution: ResolutionChordInfo): string {
  return (resolution.pitch_names || []).join(" ");
}

function frequencyForPitch(pitch: PitchInfo): number {
  const selected = pitch.tuning_frequencies?.find(
    (tuning) => tuning.id === currentSoundTuningId(),
  );
  return selected?.frequency_hz ?? pitch.frequency_hz;
}

function frequenciesFromAnalysis(analysis: ChordAnalysis | null): number[] {
  return (analysis?.pitches ?? [])
    .map(frequencyForPitch)
    .filter((frequency) => Number.isFinite(frequency) && frequency > 0);
}

function scheduleChordFrequencies(
  frequencies: number[],
  startTime: number,
  duration: number,
): void {
  const context = audioContext;
  if (!context) return;
  const gainPerVoice = Math.min(0.18, 0.52 / Math.sqrt(frequencies.length));
  const attackEnd = startTime + 0.018;
  const releaseStart = startTime + Math.max(0.08, duration - 0.1);
  const stopTime = startTime + duration + 0.08;

  for (const frequency of frequencies) {
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.type = "triangle";
    oscillator.frequency.setValueAtTime(frequency, startTime);
    gain.gain.setValueAtTime(0.0001, startTime);
    gain.gain.exponentialRampToValueAtTime(gainPerVoice, attackEnd);
    gain.gain.exponentialRampToValueAtTime(0.0001, releaseStart);
    oscillator.connect(gain).connect(context.destination);
    oscillator.start(startTime);
    oscillator.stop(stopTime);
    activeChordNodes.push(oscillator);
    oscillator.addEventListener("ended", () => {
      activeChordNodes = activeChordNodes.filter((node) => node !== oscillator);
    });
  }
}

function arpeggioDuration(frequencies: number[]): number {
  const step = 0.14;
  const noteDuration = 0.64;
  return Math.max(noteDuration, (frequencies.length - 1) * step + noteDuration);
}

function scheduleArpeggioFrequencies(frequencies: number[], startTime: number): void {
  const step = 0.14;
  const noteDuration = 0.64;

  frequencies.forEach((frequency, index) => {
    scheduleChordFrequencies([frequency], startTime + index * step, noteDuration);
  });
}

function scheduleFrequencyItem(item: FrequencySequenceItem, startTime: number): number {
  if (item.mode === "arpeggio") {
    scheduleArpeggioFrequencies(item.frequencies, startTime);
    return arpeggioDuration(item.frequencies);
  }

  scheduleChordFrequencies(item.frequencies, startTime, item.duration);
  return item.duration;
}

async function playFrequencySequence(sequence: FrequencySequenceItem[]): Promise<void> {
  const playable = sequence.filter((item) => item.frequencies.length);
  if (playable.length !== sequence.length) {
    error.textContent = "No pitched notes to play.";
    error.style.display = "block";
    return;
  }
  if (!(await ensureAudio())) return;

  stopChordPlayback();
  error.style.display = "none";
  const context = audioContext;
  if (!context) return;
  let startTime = context.currentTime;
  for (const item of playable) {
    startTime += scheduleFrequencyItem(item, startTime) + (item.gapAfter ?? 0);
  }
}

async function playCurrentChord(mode: PlaybackMode = "block"): Promise<void> {
  if (!currentAnalysis && !analyze({ remember: false })) return;
  const analysis = currentAnalysis;
  if (!analysis) return;
  const frequencies = frequenciesFromAnalysis(analysis);
  await playFrequencySequence([{ frequencies, duration: 1.45, mode }]);
}

function updatePlayChordButton(): void {
  playChord.textContent =
    nextChordPlaybackMode === "arpeggio" ? "Play arpeggio" : "Play chord";
  playChord.setAttribute(
    "aria-label",
    nextChordPlaybackMode === "arpeggio"
      ? "Play the current chord as an arpeggio"
      : "Play the current chord as a block chord",
  );
}

async function playCurrentChordFromButton(): Promise<void> {
  const mode = nextChordPlaybackMode;
  nextChordPlaybackMode = mode === "arpeggio" ? "block" : "arpeggio";
  updatePlayChordButton();
  await playCurrentChord(mode);
}

async function playResolutionPreview(resolution: ResolutionChordInfo): Promise<void> {
  if (!currentAnalysis && !analyze({ remember: false })) return;
  const analysis = currentAnalysis;
  if (!analysis) return;
  let resolutionAnalysis: ChordAnalysis;
  try {
    resolutionAnalysis = analyzeWithCurrentOptions(chordValueForResolution(resolution));
  } catch (err) {
    error.textContent = err instanceof Error ? err.message : String(err);
    error.style.display = "block";
    return;
  }

  await playFrequencySequence([
    { frequencies: frequenciesFromAnalysis(analysis), duration: 1.0, gapAfter: 0.16 },
    { frequencies: frequenciesFromAnalysis(resolutionAnalysis), duration: 1.45 },
  ]);
}

async function openResolution(resolution: ResolutionChordInfo): Promise<void> {
  const chordValue = chordValueForResolution(resolution);
  input.value = chordValue;
  resetShareButton();
  window.history.pushState({ chord: chordValue }, "", buildShareUrl(chordValue).href);
  if (analyze({ syncUrl: false, remember: true })) {
    await playCurrentChord();
  }
}

async function startMidiInput(): Promise<void> {
  if (!navigator.requestMIDIAccess) {
    setMidiStatus("Web MIDI unavailable in this browser");
    return;
  }

  setMidiStatus("Requesting MIDI access");
  try {
    midiAccess = await navigator.requestMIDIAccess({ sysex: false });
    heldMidiNotes = new Map();
    wireMidiInputs();
    const access = midiAccess;
    access.onstatechange = () => {
      wireMidiInputs();
      setMidiStatus(midiInputStatus());
    };
    setMidiStatus(midiInputStatus());
  } catch (err) {
    setMidiStatus(err instanceof Error ? err.message : "MIDI access denied");
  }
}

function wireMidiInputs(): void {
  if (!midiAccess) return;
  if (!connectedMidiInputs().length) heldMidiNotes.clear();
  for (const inputDevice of midiInputs(midiAccess)) {
    inputDevice.onmidimessage =
      inputDevice.state === "disconnected" ? null : handleMidiMessage;
  }
  setMidiStatus(midiInputStatus());
}

function midiInputStatus(): string {
  if (!midiAccess) return "MIDI idle";
  const names = connectedMidiInputs().map((inputDevice) => inputDevice.name || "MIDI input");
  if (!names.length) return "No MIDI inputs found";
  return `Listening: ${names.join(", ")}`;
}

function connectedMidiInputs(): MIDIInput[] {
  if (!midiAccess) return [];
  return midiInputs(midiAccess).filter((inputDevice) => inputDevice.state !== "disconnected");
}

function midiInputs(access: MIDIAccess): MIDIInput[] {
  return Array.from(
    (access.inputs as unknown as ReadonlyMap<string, MIDIInput>).values(),
  );
}

function handleMidiMessage(event: MIDIMessageEvent): void {
  if (!event.data) return;
  const status = event.data[0] ?? 0;
  const note = event.data[1] ?? 0;
  const velocity = event.data[2] ?? 0;
  const command = status & 0xf0;
  if (command === 0xb0 && (note === 0x7b || note === 0x7e || note === 0x7f)) {
    heldMidiNotes.clear();
    setMidiStatus(`${midiInputStatus()} - released`);
    return;
  }

  if (command === 0x90 && velocity > 0) {
    heldMidiNotes.set(note, velocity);
  } else if (command === 0x80 || (command === 0x90 && velocity === 0)) {
    heldMidiNotes.delete(note);
  } else {
    return;
  }

  analyzeHeldMidiNotes();
}

function analyzeHeldMidiNotes(): void {
  const notes = [...heldMidiNotes.keys()].sort((a, b) => a - b);
  if (!notes.length) {
    setMidiStatus(`${midiInputStatus()} - released`);
    return;
  }

  input.value = `midi: ${notes.join(" ")}`;
  resetShareButton();
  analyze({ syncUrl: false, remember: false });
  setMidiStatus(`${midiInputStatus()} - ${notes.join(" ")}`);
}

function setMidiStatus(message: string): void {
  midiStatus.textContent = message;
}

function estimateKeyContext(): void {
  let estimatedAnalysis: ChordAnalysis;
  try {
    estimatedAnalysis = analyze_chord(input.value) as ChordAnalysis;
  } catch (err) {
    error.textContent = err instanceof Error ? err.message : String(err);
    error.style.display = "block";
    return;
  }

  const estimate = normalizeKeyInput(estimatedAnalysis.key_estimate ?? "");
  if (!estimate) {
    error.textContent = "No key estimate available for this chord.";
    error.style.display = "block";
    return;
  }

  keyInput.value = estimate;
  resetShareButton();
  analyze({ remember: false });
}

function analyze({ syncUrl = true, remember = false }: AnalyzeOptions = {}): boolean {
  const chordValue = input.value;
  try {
    render(analyzeWithCurrentOptions(chordValue));
    if (syncUrl) syncShareUrl();
    if (remember) rememberChord(chordValue);
    return true;
  } catch (err) {
    error.textContent = err instanceof Error ? err.message : String(err);
    error.style.display = "block";
    return false;
  }
}

function clampInteger(value: string, min: number, max: number, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}

type RandomBoundInput = "min" | "max";

function normalizeRandomNoteBounds(changed?: RandomBoundInput): { min: number; max: number } {
  let min = clampInteger(
    randomMinNotes.value,
    randomNoteLimitMin,
    randomNoteLimitMax,
    randomNoteDefaultMin,
  );
  let max = clampInteger(
    randomMaxNotes.value,
    randomNoteLimitMin,
    randomNoteLimitMax,
    randomNoteDefaultMax,
  );
  const activeBound =
    changed ??
    (document.activeElement === randomMaxNotes
      ? "max"
      : document.activeElement === randomMinNotes
        ? "min"
        : "min");

  if (min > max) {
    if (activeBound === "max") {
      min = max;
    } else {
      max = min;
    }
  }

  randomMinNotes.value = String(min);
  randomMaxNotes.value = String(max);
  randomMinNotes.min = String(randomNoteLimitMin);
  randomMinNotes.max = String(max);
  randomMaxNotes.min = String(min);
  randomMaxNotes.max = String(randomNoteLimitMax);
  return { min, max };
}

function randomNoteCount(): number {
  const { min, max } = normalizeRandomNoteBounds();
  return min + Math.floor(Math.random() * (max - min + 1));
}

function generateRandomChord(): string {
  const rootIndex = Math.floor(Math.random() * inputPitchNames.length);
  const noteCount = randomNoteCount();
  const intervals = new Set([0]);
  while (intervals.size < noteCount) {
    intervals.add(Math.floor(Math.random() * 12));
  }
  return [...intervals]
    .sort((a, b) => a - b)
    .map((interval) => inputPitchNames[(rootIndex + interval) % inputPitchNames.length])
    .join(" ");
}

for (const value of [
  "C E G",
  "C E- G",
  "C E G#",
  "G2 B2 D3 F3",
  "midi: 60 64 67",
  "",
]) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = value === "" ? "empty" : value;
  button.addEventListener("click", () => {
    input.value = value;
    analyze({ remember: true });
  });
  examples.appendChild(button);
}

normalizeRandomNoteBounds();
randomMinNotes.addEventListener("input", () => normalizeRandomNoteBounds("min"));
randomMinNotes.addEventListener("change", () => normalizeRandomNoteBounds("min"));
randomMaxNotes.addEventListener("input", () => normalizeRandomNoteBounds("max"));
randomMaxNotes.addEventListener("change", () => normalizeRandomNoteBounds("max"));
randomChord.addEventListener("click", () => {
  input.value = generateRandomChord();
  resetShareButton();
  analyze({ remember: true });
});
clearHistory.addEventListener("click", clearChordHistory);
playChord.addEventListener("click", playCurrentChordFromButton);
estimateKey.addEventListener("click", estimateKeyContext);
updatePlayChordButton();

form.addEventListener("submit", (event) => {
  event.preventDefault();
  analyze({ remember: true });
});

input.addEventListener("input", resetShareButton);
keyInput.addEventListener("input", resetShareButton);
soundTuning.addEventListener("change", () => {
  resetShareButton();
  if (currentAnalysis) renderPitches(currentAnalysis);
  syncShareUrl();
});
guitarTuning.addEventListener("input", resetShareButton);
guitarTuning.addEventListener("change", () => {
  guitarTuning.value = currentGuitarTuning();
  resetShareButton();
  analyze({ remember: false });
});

share.addEventListener("click", async () => {
  const href = syncShareUrl();
  try {
    await writeClipboard(href);
    error.style.display = "none";
    markShareCopied();
  } catch {
    error.textContent = "The URL has been updated in your address bar.";
    error.style.display = "block";
  }
});

window.addEventListener("popstate", () => {
  const sharedChord = getSharedChord();
  if (sharedChord !== null) {
    input.value = sharedChord;
    keyInput.value = getSharedKey() ?? "";
    setSoundTuning(getSharedSoundTuning());
    setGuitarTuning(getSharedGuitarTuning());
    analyze({ syncUrl: false });
  }
});

await init();
populateSoundTuningOptions();
renderChordHistory();
const sharedChord = getSharedChord();
if (sharedChord !== null) input.value = sharedChord;
const sharedKey = getSharedKey();
if (sharedKey !== null) keyInput.value = sharedKey;
setGuitarTuning(getSharedGuitarTuning());
analyze({ syncUrl: false });
void startMidiInput();
