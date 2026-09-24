import "../help-tooltips.js";
import "../theme.js";
import init, { known_chords, pitch_class_of } from "../pkg/music21_rs_web.js";

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
    voicings: string[];
    display_voicing: string[];
    searchText?: string;
};

type RomanNumeral = {
    figure: string;
    key_context: string;
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
const chordsByRoot = new Map<number, KnownChord[]>();
let showNamedOnly = false;
let shareResetTimer: number | null = null;

const chordBaseHref =
    window.location.protocol === "file:" ? "../chord/index.html" : "../chord/";
const filterParamNames = ["q", "min", "max", "root", "named"];

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
    return root.selectedOptions[0]?.textContent ?? "C";
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

    const pitchClass = pitch_class_of(trimmed);
    return pitchClass === undefined ? null : String(pitchClass);
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

function orMissing(value: string | null | undefined): string {
    return value || "Not available";
}

function inversionLabel(index: number): string {
    if (index === 0) return "open";
    if (index % 100 >= 11 && index % 100 <= 13) return `${index}th`;
    if (index % 10 === 1) return `${index}st`;
    if (index % 10 === 2) return `${index}nd`;
    if (index % 10 === 3) return `${index}rd`;
    return `${index}th`;
}

function uniqueInversionName(
    chord: KnownChord,
    inversion: number,
): string | null {
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
        romanNumeralFor(chord),
        chord.forte_class,
        `[${(chord.normal_form ?? []).join(", ")}]`,
        `[${(chord.interval_class_vector ?? []).join(", ")}]`,
        ...(chord.display_pitch_names ?? []),
        ...chord.display_voicing,
    ]
        .join(" ")
        .toLocaleLowerCase();
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
    url.searchParams.set("chord", chord.voicings[inversion] ?? "");
    return url.href;
}

function resolutionUrl(resolution: ResolutionChord): string {
    const url = new URL(chordBaseHref, window.location.href);
    url.searchParams.set("chord", resolution.pitch_names.join(" "));
    return url.href;
}

function romanNumeralFor(chord: KnownChord): string {
    const roman = chord.roman_numeral_estimate;
    if (!roman?.figure) return "Not available";
    return roman.key_context ? `${roman.figure} in ${roman.key_context}` : roman.figure;
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
    for (const resolution of chord.resolution_chords ?? []) {
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
        return chord.searchText?.includes(query) ?? false;
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
        symbol.textContent = orMissing(chord.chord_symbol);
        tr.appendChild(symbol);

        const keyEstimate = document.createElement("td");
        keyEstimate.className = "key-estimate";
        keyEstimate.textContent = orMissing(chord.key_estimate);
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

        const pitches = document.createElement("td");
        pitches.className = "pitch-list";
        pitches.textContent = chord.display_voicing.join(" ");
        tr.appendChild(pitches);

        const inversions = document.createElement("td");
        const inversionButtons = document.createElement("div");
        inversionButtons.className = "inversion-buttons";
        for (
            let index = 1;
            index < (chord.normal_form ?? []).length;
            index += 1
        ) {
            const inversionLink = document.createElement("a");
            inversionLink.className = "mini-button";
            inversionLink.href = openUrl(chord, index);
            const label = inversionButtonLabel(chord, index);
            inversionLink.textContent = label;
            inversionLink.setAttribute(
                "aria-label",
                `Open ${label} in the chord inspector`,
            );
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

/** Loads every chord built on the chosen root, spelled by the crate, once
 * per root: building them all takes the crate most of a second. */
function loadChords(): void {
    const pitchClass = selectedRootPitchClass();
    const cached = chordsByRoot.get(pitchClass);
    if (cached) {
        allChords = cached;
        return;
    }
    allChords = (known_chords(pitchClass) as KnownChord[]).map((chord) => ({
        ...chord,
        searchText: textSearch(chord),
    }));
    chordsByRoot.set(pitchClass, allChords);
}

async function main(): Promise<void> {
    try {
        await init();
        loadChords();
        renderCardinalityOptions(allChords);
        minCardinality.value = "1";
        maxCardinality.value = "12";
        applyFiltersFromUrl();
        loadChords();
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
root.addEventListener("change", () => {
    loadChords();
    renderAndSyncFilters();
});
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
        error.textContent =
            "The filter URL has been updated in your address bar.";
        error.style.display = "block";
    }
});
window.addEventListener("popstate", () => {
    applyFiltersFromUrl();
    loadChords();
    renderRows();
});

main();
