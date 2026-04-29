import "../help-tooltips.js";
import "../theme.js";
import init, { known_chords } from "../pkg/music21_rs_web.js";

type KnownChord = {
  id: string;
  primary_common_name: string;
  common_names: string[];
  chord_symbol?: string | null;
  key_estimate?: string | null;
  roman_numeral_estimate?: RomanNumeral | null;
  resolution_chords?: ResolutionChord[];
  inversion_labels?: Array<string | null>;
  cardinality: number;
  forte_class: string;
  normal_form: number[];
  interval_class_vector: number[];
  pitch_classes?: number[];
  display_pitch_names: string[];
  searchText?: string;
};

type RomanNumeral = {
  figure: string;
  key_context: string;
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

const search = mustQuery<HTMLInputElement>("#search");
const minCardinality = mustQuery<HTMLSelectElement>("#min-cardinality");
const maxCardinality = mustQuery<HTMLSelectElement>("#max-cardinality");
const root = mustQuery<HTMLSelectElement>("#root");
const namedOnly = mustQuery<HTMLButtonElement>("#named-only");
const shareFilters = mustQuery<HTMLButtonElement>("#share-filters");
const count = mustQuery<HTMLElement>("#count");
const rows = mustQuery<HTMLTableSectionElement>("#rows");
const error = mustQuery<HTMLElement>("#error");

let allChords: KnownChord[] = [];
let showNamedOnly = false;
let shareResetTimer: number | null = null;

const chordBaseHref = window.location.pathname.endsWith(".html")
  ? "../chord/index.html"
  : "../chord/";
const filterParamNames = ["q", "min", "max", "root", "named"];
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

function selectedRootName(): string {
  return pitchDisplayNames[selectedRootPitchClass()] ?? "C";
}

function selectedRootShift(): number {
  return selectedRootPitchClass();
}

function selectHasValue(select: HTMLSelectElement, value: string): boolean {
  return Array.from(select.options).some((option) => option.value === value);
}

function rootValueFromParam(value: string | null): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;

  const numeric = Number(trimmed);
  if (Number.isInteger(numeric) && numeric >= 0 && numeric <= 11) {
    return String(numeric);
  }

  const normalized = trimmed.replaceAll("-", "b").toLocaleLowerCase();
  const index = pitchDisplayNames.findIndex(
    (name) => name.toLocaleLowerCase() === normalized,
  );
  return index >= 0 ? String(index) : null;
}

function setNamedOnly(value: boolean): void {
  showNamedOnly = value;
  namedOnly.classList.toggle("active", showNamedOnly);
  namedOnly.setAttribute("aria-pressed", String(showNamedOnly));
  namedOnly.textContent = showNamedOnly ? "Showing named" : "Named only";
}

function applyFiltersFromUrl(): void {
  const params = new URLSearchParams(window.location.search);
  search.value = params.get("q") ?? "";

  const min = params.get("min");
  if (min && selectHasValue(minCardinality, min)) {
    minCardinality.value = min;
  }

  const max = params.get("max");
  if (max && selectHasValue(maxCardinality, max)) {
    maxCardinality.value = max;
  }

  if (Number(minCardinality.value) > Number(maxCardinality.value)) {
    maxCardinality.value = minCardinality.value;
  }

  const rootValue = rootValueFromParam(params.get("root"));
  if (rootValue !== null) {
    root.value = rootValue;
  }

  setNamedOnly(
    ["1", "true", "yes"].includes(
      (params.get("named") ?? "").trim().toLocaleLowerCase(),
    ),
  );
}

function currentFilterUrl(): URL {
  const url = new URL(window.location.href);
  for (const name of filterParamNames) {
    url.searchParams.delete(name);
  }

  const query = search.value.trim();
  if (query) url.searchParams.set("q", query);
  if (minCardinality.value !== "1") {
    url.searchParams.set("min", minCardinality.value);
  }
  if (maxCardinality.value !== "12") {
    url.searchParams.set("max", maxCardinality.value);
  }
  if (selectedRootPitchClass() !== 0) {
    url.searchParams.set("root", selectedRootName());
  }
  if (showNamedOnly) {
    url.searchParams.set("named", "1");
  }

  return url;
}

function syncFilterUrl(): URL {
  const url = currentFilterUrl();
  window.history.replaceState({ chordBrowserFilters: true }, "", url);
  return url;
}

function renderAndSyncFilters(): void {
  renderRows();
  syncFilterUrl();
  resetShareButton();
}

function wrapPitchClass(pitchClass: number): number {
  return ((pitchClass % 12) + 12) % 12;
}

function pitchNameAt(midi: number, names: string[], withOctave: boolean): string {
  const pitchClass = wrapPitchClass(midi);
  if (!withOctave) return names[pitchClass];
  const octave = Math.floor(midi / 12) - 1;
  return `${names[pitchClass]}${octave}`;
}

function pitchClassForName(name: string): number | null {
  const normalized = name.replaceAll("-", "b");
  const base = normalized[0];
  const basePitchClasses: Record<string, number> = {
    C: 0,
    D: 2,
    E: 4,
    F: 5,
    G: 7,
    A: 9,
    B: 11,
  };
  const basePitchClass = basePitchClasses[base];
  if (basePitchClass === undefined) return null;

  let pitchClass = basePitchClass;
  for (const accidental of normalized.slice(1)) {
    if (accidental === "#") pitchClass += 1;
    else if (accidental === "b") pitchClass -= 1;
    else return null;
  }

  return wrapPitchClass(pitchClass);
}

function transposePitchName(
  value: string,
  shift = selectedRootShift(),
  names = pitchDisplayNames,
): string {
  const match = value.match(/^([A-G](?:[#b-]*))(-?\d+)?$/);
  if (!match) return value;

  const pitchClass = pitchClassForName(match[1]);
  if (pitchClass === null) return value;

  const octaveText = match[2];
  if (octaveText === undefined) {
    return names[wrapPitchClass(pitchClass + shift)];
  }

  const octave = Number(octaveText);
  const midi = (octave + 1) * 12 + pitchClass + shift;
  return `${names[wrapPitchClass(midi)]}${Math.floor(midi / 12) - 1}`;
}

function transposeLeadingPitchName(value: string): string {
  return value.replace(/^([A-G](?:[#b-]*))(?=-|\b)/, (name) =>
    transposePitchName(name),
  );
}

function transposeKeyText(value: string | null | undefined): string {
  if (!value) return "Not available";
  return value.replace(
    /\b([A-G](?:[#b-]*))(?=\s+(?:major|minor)\b)/g,
    (name) => transposePitchName(name),
  );
}

function transposeCChordSymbol(value: string | null | undefined): string {
  if (!value) return "Not available";
  return value.replace(/^C/, pitchDisplayNames[selectedRootPitchClass()]);
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

function uniqueInversionName(chord: KnownChord, inversion: number): string | null {
  return chord.inversion_labels?.[inversion] || null;
}

function inversionButtonLabel(chord: KnownChord, inversion: number): string {
  return uniqueInversionName(chord, inversion) ?? inversionLabel(inversion);
}

function textSearch(chord: KnownChord): string {
  return [
    chord.primary_common_name,
    ...(chord.common_names ?? []),
    chord.chord_symbol ?? "",
    chord.key_estimate ?? "",
    chord.roman_numeral_estimate?.figure ?? "",
    chord.roman_numeral_estimate?.key_context ?? "",
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
  url.searchParams.set("chord", realized.inputNames.join(" "));
  return url.href;
}

function resolutionUrl(resolution: ResolutionChord): string {
  const url = new URL(chordBaseHref, window.location.href);
  url.searchParams.set("chord", resolution.pitch_names.join(" "));
  return url.href;
}

function transposeResolutionChord(resolution: ResolutionChord): ResolutionChord {
  return {
    ...resolution,
    pitched_common_name: transposeLeadingPitchName(resolution.pitched_common_name),
    key_context: transposeKeyText(resolution.key_context),
    pitch_names: resolution.pitch_names.map((name) =>
      transposePitchName(name, selectedRootShift(), pitchInputNames),
    ),
    pitch_classes: resolution.pitch_classes.map((pitchClass) =>
      wrapPitchClass(pitchClass + selectedRootShift()),
    ),
  };
}

function chordSymbolFor(chord: KnownChord): string {
  return transposeCChordSymbol(chord.chord_symbol);
}

function keyEstimateFor(chord: KnownChord): string {
  return transposeKeyText(chord.key_estimate);
}

function romanNumeralFor(chord: KnownChord): string {
  const roman = chord.roman_numeral_estimate;
  if (!roman?.figure) return "Not available";
  const keyContext = transposeKeyText(roman.key_context);
  return keyContext === "Not available" ? roman.figure : `${roman.figure} in ${keyContext}`;
}

function resolutionChords(chord: KnownChord): ResolutionChord[] {
  return (chord.resolution_chords ?? []).map(transposeResolutionChord);
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

function resetShareButton(): void {
  shareFilters.textContent = "Copy link";
  shareFilters.classList.remove("copied");
  if (shareResetTimer !== null) {
    window.clearTimeout(shareResetTimer);
    shareResetTimer = null;
  }
}

function markShareCopied(): void {
  shareFilters.textContent = "Copied";
  shareFilters.classList.add("copied");
  if (shareResetTimer !== null) {
    window.clearTimeout(shareResetTimer);
  }
  shareResetTimer = window.setTimeout(resetShareButton, 1600);
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
      chordSymbolFor(chord),
      keyEstimateFor(chord),
      romanNumeralFor(chord),
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

    const symbol = document.createElement("td");
    symbol.className = "symbol mono";
    symbol.textContent = chordSymbolFor(chord);
    tr.appendChild(symbol);

    const keyEstimate = document.createElement("td");
    keyEstimate.className = "key-estimate";
    keyEstimate.textContent = keyEstimateFor(chord);
    tr.appendChild(keyEstimate);

    const roman = document.createElement("td");
    roman.className = "roman-numeral";
    roman.textContent = romanNumeralFor(chord);
    tr.appendChild(roman);

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
    applyFiltersFromUrl();
    syncFilterUrl();
    renderRows();
  } catch (err) {
    error.textContent = err instanceof Error ? err.message : String(err);
    error.style.display = "block";
    count.textContent = "Unavailable";
  }
}

search.addEventListener("input", renderAndSyncFilters);
minCardinality.addEventListener("change", () => {
  syncCardinalityRange("min");
  renderAndSyncFilters();
});
maxCardinality.addEventListener("change", () => {
  syncCardinalityRange("max");
  renderAndSyncFilters();
});
root.addEventListener("change", renderAndSyncFilters);
namedOnly.addEventListener("click", () => {
  setNamedOnly(!showNamedOnly);
  renderAndSyncFilters();
});
shareFilters.addEventListener("click", async () => {
  const url = syncFilterUrl();
  try {
    await writeClipboard(url.href);
    error.style.display = "none";
    markShareCopied();
  } catch {
    error.textContent = "The filter URL has been updated in your address bar.";
    error.style.display = "block";
  }
});
window.addEventListener("popstate", () => {
  applyFiltersFromUrl();
  renderRows();
});

main();
