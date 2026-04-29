import "../help-tooltips.js";
import init, { analyze_chord, known_chords } from "../pkg/music21_rs_web.js";

type KnownChord = {
  id: string;
  primary_common_name: string;
  common_names: string[];
  cardinality: number;
  forte_class: string;
  normal_form: number[];
  interval_class_vector: number[];
  pitch_classes?: number[];
  display_pitch_names: string[];
  searchText?: string;
};

type RealizedChord = {
  midi: number[];
  inputNames: string[];
  displayNames: string[];
};

type CardinalityRange = {
  minimum: number;
  maximum: number;
};

type ResolutionChord = {
  pitched_common_name: string;
  key_context: string;
  pitch_names: string[];
  pitch_classes: number[];
};

type ChordAnalysis = {
  common_name?: string;
  resolution_chords?: ResolutionChord[];
};

const search = mustQuery<HTMLInputElement>("#search");
const minCardinality = mustQuery<HTMLSelectElement>("#min-cardinality");
const maxCardinality = mustQuery<HTMLSelectElement>("#max-cardinality");
const root = mustQuery<HTMLSelectElement>("#root");
const namedOnly = mustQuery<HTMLButtonElement>("#named-only");
const count = mustQuery<HTMLElement>("#count");
const rows = mustQuery<HTMLTableSectionElement>("#rows");
const error = mustQuery<HTMLElement>("#error");

let allChords: KnownChord[] = [];
let showNamedOnly = false;
const resolutionCache = new Map<string, ResolutionChord[]>();
const inversionLabelCache = new Map<string, string>();

const chordBaseHref = window.location.pathname.endsWith(".html")
  ? "../chord/index.html"
  : "../chord/";
const pitchInputNames = [
  "C",
  "D-",
  "D",
  "E-",
  "E",
  "F",
  "F#",
  "G",
  "A-",
  "A",
  "B-",
  "B",
];
const pitchDisplayNames = [
  "C",
  "Db",
  "D",
  "Eb",
  "E",
  "F",
  "F#",
  "G",
  "Ab",
  "A",
  "Bb",
  "B",
];

function mustQuery<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`Missing required element: ${selector}`);
  }
  return element;
}

function selectedRootPitchClass(): number {
  return Number(root.value) || 0;
}

function pitchNameAt(midi: number, names: string[], withOctave: boolean): string {
  const pitchClass = ((midi % 12) + 12) % 12;
  if (!withOctave) return names[pitchClass];
  const octave = Math.floor(midi / 12) - 1;
  return `${names[pitchClass]}${octave}`;
}

function realizedChord(chord: KnownChord, inversion = 0): RealizedChord {
  const pitchClasses = chord.pitch_classes ?? chord.normal_form ?? [];
  if (pitchClasses.length === 0) {
    return { midi: [], inputNames: [], displayNames: [] };
  }

  const rootMidi = 60 + selectedRootPitchClass();
  const rootPosition = pitchClasses.map((offset) => rootMidi + offset);
  const rotation = inversion % rootPosition.length;
  const midi = rootPosition
    .slice(rotation)
    .concat(rootPosition.slice(0, rotation).map((value) => value + 12));

  return {
    midi,
    inputNames: midi.map((value) => pitchNameAt(value, pitchInputNames, true)),
    displayNames: midi.map((value) =>
      pitchNameAt(value, pitchDisplayNames, true),
    ),
  };
}

function inversionLabel(index: number): string {
  if (index === 0) return "open";
  if (index % 100 >= 11 && index % 100 <= 13) return `${index}th`;
  if (index % 10 === 1) return `${index}st`;
  if (index % 10 === 2) return `${index}nd`;
  if (index % 10 === 3) return `${index}rd`;
  return `${index}th`;
}

function normalizedChordName(name: string): string {
  return name.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

function displayInversionName(name: string): string {
  const normalized = name.trim().replace(/\s+/g, " ");
  return normalized.replace(/^\S/, (letter) => letter.toLocaleLowerCase());
}

function uniqueInversionName(chord: KnownChord, inversion: number): string | null {
  const realized = realizedChord(chord, inversion);
  if (!realized.midi.length) return null;

  const cacheKey = `${selectedRootPitchClass()}:${chord.id}:${inversion}`;
  const cached = inversionLabelCache.get(cacheKey);
  if (cached !== undefined) return cached || null;

  try {
    const analysis = analyze_chord(`midi: ${realized.midi.join(" ")}`) as ChordAnalysis;
    const commonName = analysis.common_name?.trim();
    if (!commonName || normalizedChordName(commonName) === "unknown chord") {
      inversionLabelCache.set(cacheKey, "");
      return null;
    }

    const ownNames = new Set(
      [chord.primary_common_name, ...(chord.common_names ?? [])].map(
        normalizedChordName,
      ),
    );
    if (!ownNames.has(normalizedChordName(commonName))) {
      const label = displayInversionName(commonName);
      inversionLabelCache.set(cacheKey, label);
      return label;
    }
  } catch {
    // Fall back to the plain inversion number below.
  }

  inversionLabelCache.set(cacheKey, "");
  return null;
}

function inversionButtonLabel(chord: KnownChord, inversion: number): string {
  return uniqueInversionName(chord, inversion) ?? inversionLabel(inversion);
}

function textSearch(chord: KnownChord): string {
  return [
    chord.primary_common_name,
    ...(chord.common_names ?? []),
    chord.forte_class,
    `[${(chord.normal_form ?? []).join(", ")}]`,
    `[${(chord.interval_class_vector ?? []).join(", ")}]`,
    ...(chord.display_pitch_names ?? []),
  ]
    .join(" ")
    .toLocaleLowerCase();
}

function directedIntervalSpan(chord: KnownChord): number {
  const pitchClasses = chord.pitch_classes ?? chord.normal_form ?? [];
  if (pitchClasses.length < 2) return 0;
  return (pitchClasses[1] - pitchClasses[0] + 12) % 12;
}

function compareChordsForBrowser(a: KnownChord, b: KnownChord): number {
  if (a.cardinality !== b.cardinality) {
    return a.cardinality - b.cardinality;
  }
  if (a.cardinality === 2 && b.cardinality === 2) {
    return directedIntervalSpan(a) - directedIntervalSpan(b);
  }
  return 0;
}

function renderCardinalityOptions(chords: KnownChord[]): void {
  const sizes = [...new Set(chords.map((chord) => chord.cardinality))]
    .filter(Number.isFinite)
    .sort((a, b) => a - b);
  for (const size of sizes) {
    for (const select of [minCardinality, maxCardinality]) {
      const option = document.createElement("option");
      option.value = String(size);
      option.textContent = `${size} note${size === 1 ? "" : "s"}`;
      select.appendChild(option);
    }
  }
}

function selectedCardinalityRange(): CardinalityRange {
  const minimum = Number(minCardinality.value);
  const maximum = Number(maxCardinality.value);
  return { minimum, maximum };
}

function syncCardinalityRange(changed: "min" | "max"): void {
  const minimum = Number(minCardinality.value);
  const maximum = Number(maxCardinality.value);
  if (changed === "min" && minimum > maximum) {
    maxCardinality.value = minCardinality.value;
  } else if (changed === "max" && maximum < minimum) {
    minCardinality.value = maxCardinality.value;
  }
}

function openUrl(chord: KnownChord, inversion = 0): string {
  const url = new URL(chordBaseHref, window.location.href);
  const realized = realizedChord(chord, inversion);
  url.searchParams.set("chord", `midi: ${realized.midi.join(" ")}`);
  return url.href;
}

function resolutionUrl(resolution: ResolutionChord): string {
  const url = new URL(chordBaseHref, window.location.href);
  url.searchParams.set("chord", resolution.pitch_names.join(" "));
  return url.href;
}

function resolutionChords(chord: KnownChord): ResolutionChord[] {
  const realized = realizedChord(chord, 0);
  if (!realized.midi.length) return [];

  const cacheKey = `${selectedRootPitchClass()}:${chord.id}`;
  const cached = resolutionCache.get(cacheKey);
  if (cached) return cached;

  try {
    const analysis = analyze_chord(`midi: ${realized.midi.join(" ")}`) as ChordAnalysis;
    const resolutions = analysis.resolution_chords ?? [];
    resolutionCache.set(cacheKey, resolutions);
    return resolutions;
  } catch {
    resolutionCache.set(cacheKey, []);
    return [];
  }
}

function renderChips(values: string[]): HTMLDivElement {
  const wrap = document.createElement("div");
  wrap.className = "chips";
  for (const value of values) {
    const chip = document.createElement("span");
    chip.className = "chip";
    chip.textContent = value;
    wrap.appendChild(chip);
  }
  return wrap;
}

function renderResolutions(chord: KnownChord): HTMLDivElement {
  const wrap = document.createElement("div");
  wrap.className = "resolution-links";
  for (const resolution of resolutionChords(chord)) {
    const link = document.createElement("a");
    link.className = "mini-button";
    link.href = resolutionUrl(resolution);
    link.textContent = resolution.pitched_common_name;
    link.title = resolution.key_context;
    wrap.appendChild(link);
  }
  return wrap;
}

function renderRows(): void {
  const query = search.value.trim().toLocaleLowerCase();
  const { minimum, maximum } = selectedCardinalityRange();
  const filtered = allChords.filter((chord) => {
    if (chord.cardinality < minimum) return false;
    if (chord.cardinality > maximum) return false;
    if (showNamedOnly && !chord.common_names.length) return false;
    if (!query) return true;
    const realized = realizedChord(chord, 0);
    const realizedWithoutOctaves = realized.displayNames.map((name) =>
      name.replace(/\d+$/, ""),
    );
    const dynamicSearchText = [
      chord.searchText,
      realized.displayNames.join(" "),
      realizedWithoutOctaves.join(" "),
    ]
      .join(" ")
      .toLocaleLowerCase();
    return dynamicSearchText.includes(query);
  });

  rows.replaceChildren();
  for (const chord of filtered) {
    const tr = document.createElement("tr");

    const name = document.createElement("td");
    name.className = "name";
    const nameLink = document.createElement("a");
    nameLink.className = "open-link";
    nameLink.href = openUrl(chord);
    nameLink.textContent = chord.primary_common_name;
    name.appendChild(nameLink);
    tr.appendChild(name);

    const aliases = document.createElement("td");
    aliases.appendChild(renderChips(chord.common_names.slice(1)));
    tr.appendChild(aliases);

    const forte = document.createElement("td");
    forte.className = "mono";
    forte.textContent = chord.forte_class;
    tr.appendChild(forte);

    const realized = realizedChord(chord, 0);
    const pitches = document.createElement("td");
    pitches.className = "pitch-list";
    pitches.textContent = realized.displayNames.join(" ");
    tr.appendChild(pitches);

    const inversions = document.createElement("td");
    const inversionButtons = document.createElement("div");
    inversionButtons.className = "inversion-buttons";
    for (let index = 1; index < (chord.normal_form ?? []).length; index += 1) {
      const inversionLink = document.createElement("a");
      inversionLink.className = "mini-button";
      inversionLink.href = openUrl(chord, index);
      const label = inversionButtonLabel(chord, index);
      inversionLink.textContent = label;
      inversionLink.setAttribute("aria-label", `Open ${label} in the chord inspector`);
      inversionButtons.appendChild(inversionLink);
    }
    inversions.appendChild(inversionButtons);
    tr.appendChild(inversions);

    const resolutions = document.createElement("td");
    resolutions.appendChild(renderResolutions(chord));
    tr.appendChild(resolutions);

    const normal = document.createElement("td");
    normal.className = "mono";
    normal.textContent = `[${chord.normal_form.join(", ")}]`;
    tr.appendChild(normal);

    const vector = document.createElement("td");
    vector.className = "mono";
    vector.textContent = `[${chord.interval_class_vector.join(", ")}]`;
    tr.appendChild(vector);

    rows.appendChild(tr);
  }

  count.textContent = `${filtered.length} of ${allChords.length} chord${
    allChords.length === 1 ? "" : "s"
  }`;
}

async function main(): Promise<void> {
  try {
    await init();
    allChords = (known_chords() as KnownChord[]).map((chord) => ({
      ...chord,
      searchText: textSearch(chord),
    }));
    allChords.sort(compareChordsForBrowser);
    renderCardinalityOptions(allChords);
    minCardinality.value = "1";
    maxCardinality.value = "12";
    renderRows();
  } catch (err) {
    error.textContent = err instanceof Error ? err.message : String(err);
    error.style.display = "block";
    count.textContent = "Unavailable";
  }
}

search.addEventListener("input", renderRows);
minCardinality.addEventListener("change", () => {
  syncCardinalityRange("min");
  renderRows();
});
maxCardinality.addEventListener("change", () => {
  syncCardinalityRange("max");
  renderRows();
});
root.addEventListener("change", renderRows);
namedOnly.addEventListener("click", () => {
  showNamedOnly = !showNamedOnly;
  namedOnly.classList.toggle("active", showNamedOnly);
  namedOnly.setAttribute("aria-pressed", String(showNamedOnly));
  namedOnly.textContent = showNamedOnly ? "Showing named" : "Named only";
  renderRows();
});

main();
