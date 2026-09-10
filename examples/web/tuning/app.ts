// @ts-nocheck
import "../theme.js";

// ---------------------------------------------------------------- elements

const $ = (selector) => document.querySelector(selector);
const searchInput = $("#search");
const chips = $("#chips");
const picker = $("#picker");
const titleNode = $("#title");
const factsNode = $("#facts");
const sizesNode = $("#sizes");
const browseButton = $("#browse");
const keysNode = $("#keys");
const layoutSelect = $("#layout");
const transposeInput = $("#transpose");
const waveform = $("#waveform");
const volume = $("#volume");
const playScaleButton = $("#play-scale");
const stopButton = $("#stop");
const midiButton = $("#midi");
const statusNode = $("#status");
const degreesBody = $("#degrees");
const aboutNode = $("#about");
const shareButton = $("#share");
const errorNode = $("#error");

$("#docs-link").href = "../docs/music21_rs/index.html";

// ---------------------------------------------------------------- constants

const defaultRootFrequency = 261.6256;
const adaptiveId = "adaptive:recursive";
const flatNames = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];
const majorScaleSemitones = [0, 2, 4, 5, 7, 9, 11, 12];

// Physical key positions, so the layout is the same on QWERTZ and AZERTY
// keyboards: what matters is where a key sits, not what it prints.
const pianoLower = [
    "KeyZ", "KeyS", "KeyX", "KeyD", "KeyC", "KeyV", "KeyG", "KeyB", "KeyH",
    "KeyN", "KeyJ", "KeyM", "Comma", "KeyL", "Period", "Semicolon", "Slash",
];
const pianoUpper = [
    "KeyQ", "Digit2", "KeyW", "Digit3", "KeyE", "KeyR", "Digit5", "KeyT",
    "Digit6", "KeyY", "Digit7", "KeyU", "KeyI", "Digit9", "KeyO", "Digit0",
    "KeyP", "BracketLeft", "Equal", "BracketRight",
];
const keyRows = [
    ["KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash"],
    ["KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote"],
    ["KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight"],
    ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal"],
];
const captions = {
    Comma: ",", Period: ".", Slash: "/", Semicolon: ";", Quote: "'",
    BracketLeft: "[", BracketRight: "]", Minus: "-", Equal: "=",
};

// ---------------------------------------------------------------- state

let wasm = null;
let systems = [];
let temperaments = [];
let equalPresets = [];
let current = null;
let selectedId = "";
let rootHz = defaultRootFrequency;
let kind = "all";
let audio = null;
let voices = new Map();
let sequence = [];
let midiAccess = null;
let shareTimer = null;

// ---------------------------------------------------------------- helpers

function fail(message) {
    errorNode.textContent = message;
    errorNode.style.display = "block";
}

function clearError() {
    errorNode.style.display = "none";
}

function clamp(value, min, max, fallback) {
    const parsed = Number.parseFloat(value);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.min(max, Math.max(min, parsed));
}

function hz(value) {
    return Number(value).toFixed(value >= 100 ? 2 : 3);
}

function signed(value, digits = 2) {
    const text = Number(value).toFixed(digits);
    return value > 0 ? `+${text}` : text;
}

function steps(system = current) {
    return Math.max(1, Number(system?.octave_size) || 12);
}

function octaveRepeating(system = current) {
    return Math.abs(Number(system?.period_ratio ?? 2) - 2) < 1e-9;
}

function twelveTone(system = current) {
    return steps(system) === 12 && octaveRepeating(system);
}

function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
}

// ---------------------------------------------------------------- shaping

/// Every scale the page shows has this shape: `degrees` runs from the root
/// to the period, and `period_ratio` says how the scale repeats.
function fromScala(info, family, id, name) {
    const count = Math.max(1, info.degree_count);
    const equalStep = info.period_cents / count;
    return {
        id,
        name,
        family,
        description: info.description || "",
        octave_size: info.degree_count,
        period_ratio: info.period_ratio,
        period_cents: info.period_cents,
        root_frequency_hz: info.root_frequency_hz,
        degrees: info.degrees.map((degree) => ({
            degree: degree.degree,
            label: degree.label,
            ratio: degree.ratio,
            ratio_label: degree.is_exact_ratio ? degree.label : `${degree.cents.toFixed(3)}¢`,
            cents: degree.cents,
            frequency_hz: degree.frequency_hz,
            from_equal: degree.cents - degree.degree * equalStep,
        })),
    };
}

function fromBuiltIn(system) {
    return {
        ...system,
        period_ratio: 2,
        period_cents: 1200,
        degrees: system.degrees.map((degree) => ({
            ...degree,
            cents: 1200 * Math.log2(degree.ratio),
            from_equal: degree.cents_from_equal_temperament,
        })),
    };
}

function adaptiveSystem(frequency) {
    const degrees = [];
    for (let degree = 0; degree <= 12; degree += 1) {
        const value = wasm.adaptive_frequency(0, degree, frequency);
        const cents = 1200 * Math.log2(value / frequency);
        degrees.push({
            degree,
            label: `${flatNames[degree % 12]}${4 + Math.floor(degree / 12)}`,
            ratio: value / frequency,
            ratio_label: `${cents.toFixed(3)}¢`,
            cents,
            frequency_hz: value,
            from_equal: cents - degree * 100,
        });
    }
    return {
        id: adaptiveId,
        name: "Recursive just intonation",
        family: "Adaptive",
        description:
            "Every key is tuned in just intonation from the lowest key held, so the same key sounds at a different frequency in a different chord. The table shows the tuning over C; hold a bass note and the rest retune to it.",
        octave_size: 12,
        period_ratio: 2,
        period_cents: 1200,
        root_frequency_hz: frequency,
        adaptive: true,
        degrees,
    };
}

/// The scale a share id names, built from whichever library holds it.
function systemFor(id) {
    if (!id) return null;
    if (id === adaptiveId) return adaptiveSystem(rootHz);
    if (id.startsWith("scala:")) {
        const file = id.slice("scala:".length);
        return fromScala(wasm.scala_scale(file, rootHz), "Scala archive", id, file.replace(/\.scl$/, ""));
    }
    if (id.startsWith("temperament:")) {
        const [, name, notes] = id.split(":");
        const info = wasm.temperament_scale(name, Number.parseInt(notes, 10), rootHz);
        const entry = temperaments.find((candidate) => candidate.name === name) ?? null;
        const system = fromScala(info, "Regular temperament", id, `${entry?.page ?? name}, ${notes} notes`);
        system.temperament = entry;
        return system;
    }
    if (id.startsWith("equal:")) {
        const [, divisions, numerator, denominator] = id.split(":").map(Number);
        const info = wasm.equal_division_scale(divisions, numerator, denominator, rootHz);
        return fromScala(info, "Equal division", id, info.file);
    }
    const builtIn = systems.find((system) => system.id === id);
    return builtIn ? fromBuiltIn(builtIn) : null;
}

// ---------------------------------------------------------------- loading

async function loadLibraries() {
    if (!wasm) {
        const module = await import(new URL("../pkg/music21_rs_web.js", window.location.href).href);
        await module.default();
        wasm = module;
    }
    systems = wasm.tuning_systems(rootHz);
    temperaments = typeof wasm.regular_temperaments === "function" ? wasm.regular_temperaments() : [];
    equalPresets = typeof wasm.equal_division_presets === "function" ? wasm.equal_division_presets() : [];
    $("#count-system").textContent = ` ${systems.length + 1}`;
    $("#count-temperament").textContent = ` ${temperaments.length}`;
    $("#count-equal").textContent = ` ${equalPresets.length}`;
    $("#count-scala").textContent =
        typeof wasm.scala_archive_len === "function" ? ` ${wasm.scala_archive_len().toLocaleString()}` : "";
}

async function start() {
    try {
        await loadLibraries();
        clearError();
        select(selectedId || systems[0]?.id || "", { pushUrl: false });
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

function select(id, { pushUrl = true } = {}) {
    let system = null;
    try {
        system = systemFor(id);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
    if (!system) {
        if (id !== systems[0]?.id) return select(systems[0]?.id ?? "", { pushUrl });
        return;
    }
    adopt(system, { pushUrl });
}

function adopt(system, { pushUrl = true } = {}) {
    stopSequence();
    releaseAll();
    current = system;
    selectedId = system.id;
    clearError();
    document.body.classList.remove("browsing");
    renderPicker();
    renderScale();
    if (pushUrl) syncUrl();
}

/// Re-realizes the current scale after the root changed.
function reroot() {
    systems = wasm.tuning_systems(rootHz);
    if (current?.id.startsWith("pasted:")) {
        current = {
            ...current,
            root_frequency_hz: rootHz,
            degrees: current.degrees.map((degree) => ({ ...degree, frequency_hz: rootHz * degree.ratio })),
        };
        renderScale();
        syncUrl();
        return;
    }
    select(selectedId);
}

// ---------------------------------------------------------------- picker

function item(name, stepsText, sub, id, onClick) {
    const node = el("button", `item${id === selectedId ? " active" : ""}`);
    node.type = "button";
    node.append(el("span", "name", name), el("span", "steps", stepsText), el("span", "sub", sub));
    node.addEventListener("click", onClick);
    return node;
}

function group(title, count) {
    const node = el("div", "group");
    node.append(el("span", "", title), el("span", "", count));
    return node;
}

function matches(text, query) {
    return !query || text.toLowerCase().includes(query);
}

function preferredSize(entry) {
    const sizes = entry.moments;
    if (!sizes.length) return null;
    return sizes.find((size) => size >= 5 && size <= 12) ?? sizes[sizes.length - 1];
}

function renderPicker() {
    const query = searchInput.value.trim().toLowerCase();
    const fragment = document.createDocumentFragment();

    if (kind === "equal") fragment.appendChild(equalForm());
    if (kind === "mine") fragment.appendChild(mineForm());

    if (kind === "all" || kind === "system") {
        const rows = [];
        for (const system of systems) {
            if (!matches(`${system.name} ${system.family} ${system.description}`, query)) continue;
            rows.push(item(system.name, `${system.octave_size} / oct`, `${system.family} · ${system.description}`, system.id, () => select(system.id)));
        }
        if (matches("recursive just intonation adaptive", query)) {
            rows.push(item("Recursive just intonation", "12 / oct", "Adaptive · every key tuned from the lowest one held", adaptiveId, () => select(adaptiveId)));
        }
        if (rows.length) {
            fragment.appendChild(group("Tuning systems", rows.length));
            for (const row of rows) fragment.appendChild(row);
        }
    }

    if (kind === "all" || kind === "temperament") {
        const rows = [];
        for (const entry of temperaments) {
            const haystack = `${entry.name} ${entry.page} ${entry.subgroup} ${entry.commas.join(" ")} ${entry.published_moments.join(" ")}`;
            if (!matches(haystack, query)) continue;
            const generators = entry.generators.map((g) => `${g.ratio} ${g.cents.toFixed(1)}¢`).join(", ");
            const sizesText = entry.moments.length ? `${entry.moments.slice(0, 7).join(" ")}${entry.moments.length > 7 ? " …" : ""}` : "rank 3";
            const active = current?.temperament?.name === entry.name;
            const row = item(entry.page, sizesText, `${entry.subgroup} · generator ${generators}`, active ? selectedId : "", () => {
                const size = preferredSize(entry);
                if (size === null) {
                    fail(`${entry.page} is rank ${entry.rank}: it stacks two generators, so it has no single moment-of-symmetry scale to play. Its mapping and commas are on the wiki page.`);
                    return;
                }
                select(`temperament:${entry.name}:${size}`);
            });
            if (active) row.classList.add("active");
            rows.push(row);
        }
        if (rows.length) {
            fragment.appendChild(group("Regular temperaments", rows.length));
            for (const row of rows) fragment.appendChild(row);
        }
    }

    if (kind === "all" || kind === "equal") {
        const rows = [];
        for (const preset of equalPresets) {
            if (!matches(`${preset.label} ${preset.note} edo edt equal`, query)) continue;
            const id = `equal:${preset.divisions}:${preset.numerator}:${preset.denominator}`;
            rows.push(item(preset.label, `${preset.divisions} / ${preset.numerator}:${preset.denominator}`, preset.note, id, () => select(id)));
        }
        if (rows.length) {
            fragment.appendChild(group("Equal divisions", rows.length));
            for (const row of rows) fragment.appendChild(row);
        }
    }

    if ((kind === "all" && query) || kind === "scala") {
        if (wasm && typeof wasm.scala_archive_index === "function") {
            const found = wasm.scala_archive_index(query, 0);
            fragment.appendChild(group("Scala archive", found.length.toLocaleString()));
            for (const entry of found) {
                const id = `scala:${entry.file}`;
                fragment.appendChild(item(entry.file.replace(/\.scl$/, ""), `${entry.degree_count} / period`, entry.description || "(no description)", id, () => select(id)));
            }
        }
    } else if (kind === "all" && wasm && typeof wasm.scala_archive_len === "function") {
        fragment.appendChild(group("Scala archive", wasm.scala_archive_len().toLocaleString()));
        fragment.appendChild(el("div", "note", "Type to search the archive, or pick the Scala filter to list every scale."));
    }

    if (!fragment.childElementCount) {
        fragment.appendChild(el("div", "note", query ? `Nothing matches "${searchInput.value.trim()}".` : "Nothing here yet."));
    }
    picker.replaceChildren(fragment);
}

function equalForm() {
    const form = el("div", "picker-form");
    const divisions = el("label", "", "Divisions");
    const divisionsInput = el("input");
    divisionsInput.type = "number";
    divisionsInput.min = "1";
    divisionsInput.max = "1000";
    divisionsInput.value = "19";
    divisions.appendChild(divisionsInput);
    const ratio = el("label", "", "Of the interval");
    const ratioInput = el("input");
    ratioInput.type = "text";
    ratioInput.value = "2/1";
    ratioInput.placeholder = "2/1, 3/1, 3/2";
    ratio.appendChild(ratioInput);
    const go = el("button", "", "Use this division");
    go.type = "button";
    go.addEventListener("click", () => {
        const count = Math.round(clamp(divisionsInput.value, 1, 1000, 12));
        const [top, bottom = "1"] = String(ratioInput.value).trim().split("/");
        const numerator = Number.parseInt(top, 10);
        const denominator = Number.parseInt(bottom, 10);
        if (!(numerator > 0) || !(denominator > 0)) {
            fail(`${ratioInput.value} is not a ratio such as 3/2.`);
            return;
        }
        select(`equal:${count}:${numerator}:${denominator}`);
    });
    const actions = el("div", "row-actions");
    actions.appendChild(go);
    form.append(divisions, ratio, actions);
    return form;
}

function mineForm() {
    const form = el("div", "picker-form");
    const fileLabel = el("label", "", "Scala file");
    const file = el("input");
    file.type = "file";
    file.accept = ".scl,text/plain";
    file.addEventListener("change", async () => {
        const chosen = file.files?.[0];
        if (!chosen) return;
        loadPasted(chosen.name, await chosen.text());
    });
    fileLabel.appendChild(file);
    const textLabel = el("label", "", "Or paste its text");
    const text = el("textarea");
    text.rows = 6;
    text.spellcheck = false;
    text.placeholder = "! my.scl\n!\nMy scale\n 3\n!\n 9/8\n 3/2\n 2/1";
    textLabel.appendChild(text);
    const go = el("button", "", "Use pasted scale");
    go.type = "button";
    go.addEventListener("click", () => loadPasted("pasted.scl", text.value));
    const actions = el("div", "row-actions");
    actions.appendChild(go);
    form.append(fileLabel, textLabel, actions);
    return form;
}

function loadPasted(fileName, contents) {
    if (!contents.trim()) {
        fail("Paste the contents of a .scl file first.");
        return;
    }
    try {
        const info = wasm.parse_scala_scale(fileName, contents, rootHz);
        adopt(fromScala(info, "Your scale", `pasted:${fileName}`, fileName.replace(/\.scl$/, "")));
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

// ---------------------------------------------------------------- scale

function periodText(system) {
    const ratio = Number(system.period_ratio);
    if (Math.abs(ratio - 2) < 1e-9) return "octave";
    if (Math.abs(ratio - 3) < 1e-9) return "tritave 3/1";
    return `${ratio.toFixed(4)} (${Number(system.period_cents).toFixed(1)}¢)`;
}

function factNode(label, value) {
    const node = el("span");
    node.append(`${label} `);
    if (value instanceof Node) node.appendChild(value);
    else node.appendChild(el("b", "", value));
    return node;
}

function renderScale() {
    const system = current;
    if (!system) return;
    titleNode.textContent = system.name;

    const root = el("input");
    root.type = "number";
    root.min = "20";
    root.max = "2000";
    root.step = "0.0001";
    root.value = String(rootHz);
    root.title = "The frequency of the root, C4";
    root.addEventListener("change", () => {
        rootHz = clamp(root.value, 20, 2000, defaultRootFrequency);
        reroot();
    });
    const rootFact = el("span");
    rootFact.append("root ", root, " Hz");
    factsNode.replaceChildren(
        factNode("", system.family),
        factNode("", `${steps(system)} steps per ${periodText(system)}`),
        rootFact,
    );

    sizesNode.replaceChildren();
    const entry = system.temperament;
    if (entry) {
        sizesNode.append("Notes per equave");
        for (const size of entry.moments) {
            const id = `temperament:${entry.name}:${size}`;
            const chip = el("button", id === system.id ? "active" : "", String(size));
            chip.type = "button";
            chip.addEventListener("click", () => select(id));
            sizesNode.appendChild(chip);
        }
    }

    renderKeys();
    renderDegrees();
    renderAbout();
}

function renderDegrees() {
    const system = current;
    degreesBody.replaceChildren();
    for (const degree of system.degrees) {
        const row = el("tr");
        row.dataset.degree = String(degree.degree);
        for (const value of [degree.degree, nameOf(degree.degree), degree.ratio_label, hz(degree.frequency_hz)]) {
            row.appendChild(el("td", "", String(value)));
        }
        const cents = el("td");
        cents.appendChild(centsBar(degree.from_equal));
        row.appendChild(cents);
        const play = el("td", "play");
        const playButton = el("button", "secondary", "Play");
        playButton.type = "button";
        playButton.addEventListener("click", () => playOnce(degree.degree));
        play.appendChild(playButton);
        row.appendChild(play);
        degreesBody.appendChild(row);
    }
}

function centsBar(cents) {
    const wrapper = el("div", "cents");
    const track = el("span", "cents-track");
    const fill = el("span", `cents-fill${cents < 0 ? " negative" : ""}`);
    fill.style.width = `${Math.min(50, Math.abs(cents) * 1.5)}%`;
    track.appendChild(fill);
    wrapper.append(el("span", "", `${signed(cents)}¢`), track);
    return wrapper;
}

function renderAbout() {
    const system = current;
    aboutNode.replaceChildren();
    if (system.description) aboutNode.appendChild(el("p", "", system.description));
    const entry = system.temperament;
    if (entry) {
        aboutNode.appendChild(el("p", "", `Subgroup ${entry.subgroup}, rank ${entry.rank}, ${entry.periods_per_equave} period${entry.periods_per_equave === 1 ? "" : "s"} to the equave. Generators: ${entry.generators.map((g) => `${g.ratio} = ${g.cents.toFixed(3)}¢`).join(", ")} (${entry.optimization}). Tempers out ${entry.commas.join(", ")}.`));
        if (entry.published_moments.length) {
            aboutNode.appendChild(el("p", "", `Scales the wiki lists: ${entry.published_moments.join(", ")}.`));
        }
        const link = el("a", "", `${entry.page} on the Xenharmonic Wiki`);
        link.href = `https://en.xen.wiki/w/${encodeURIComponent(entry.page)}`;
        link.target = "_blank";
        link.rel = "noreferrer";
        const paragraph = el("p");
        paragraph.appendChild(link);
        aboutNode.appendChild(paragraph);
    }
}

/// A degree's name: note names for a twelve-tone octave scale, the scale's
/// own label otherwise, with the period offset appended past the first.
function nameOf(degree) {
    const system = current;
    const count = steps(system);
    const periods = Math.floor(degree / count);
    const step = degree - periods * count;
    const octave = periods + octaves();
    if (twelveTone(system)) return `${flatNames[step]}${4 + octave}`;
    const entry = system.degrees[step];
    const label = entry ? entry.label : String(step);
    return octave === 0 ? label : `${label} ${octave > 0 ? "+" : ""}${octave}`;
}

// ---------------------------------------------------------------- keyboard

function octaves() {
    return Math.round(clamp(transposeInput.value, -4, 4, 0));
}

function layout() {
    const chosen = layoutSelect.value;
    if (chosen !== "auto") return chosen;
    return twelveTone() ? "piano" : "rows";
}

/// Which degree each physical key plays under the layout in force.
function keymap() {
    const count = steps();
    const map = new Map();
    const mode = layout();
    if (mode === "piano") {
        pianoLower.forEach((code, index) => map.set(code, index));
        pianoUpper.forEach((code, index) => map.set(code, index + 12));
    } else if (mode === "periods") {
        keyRows.forEach((row, rowIndex) => {
            row.forEach((code, index) => {
                if (index <= count) map.set(code, rowIndex * count + index);
            });
        });
    } else {
        let degree = 0;
        for (const row of keyRows) {
            for (const code of row) map.set(code, degree++);
        }
    }
    return map;
}

function caption(code) {
    if (captions[code]) return captions[code];
    if (code.startsWith("Key")) return code.slice(3).toLowerCase();
    if (code.startsWith("Digit")) return code.slice(5);
    return code;
}

function lowestHeld() {
    let lowest = null;
    for (const voice of voices.values()) {
        if (lowest === null || voice.degree < lowest) lowest = voice.degree;
    }
    return lowest;
}

/// The frequency a keyboard degree sounds: counted from the root in steps of
/// the scale, wrapping at the period, shifted by the octave control.
function frequencyOf(degree) {
    const system = current;
    if (!system) return 0;
    const count = steps(system);
    const shifted = degree + octaves() * count;
    if (system.adaptive) {
        const held = lowestHeld();
        const context = held === null ? shifted : held + octaves() * count;
        return wasm.adaptive_frequency(context, shifted, rootHz);
    }
    const periods = Math.floor(shifted / count);
    const step = shifted - periods * count;
    return rootHz * Math.pow(system.period_ratio, periods) * (system.degrees[step]?.ratio ?? 1);
}

function renderKeys() {
    keysNode.replaceChildren();
    const system = current;
    if (!system) return;
    const count = steps(system);
    const hints = new Map();
    for (const [code, degree] of keymap()) {
        if (!hints.has(degree)) hints.set(degree, caption(code));
    }
    const shown = Math.min(count * 3 + 1, Math.max(count * 2 + 1, 25));
    const fragment = document.createDocumentFragment();
    for (let degree = 0; degree < shown; degree += 1) {
        const step = degree % count;
        const dark = twelveTone(system) && [1, 3, 6, 8, 10].includes(step);
        const key = el("button", `key${step === 0 ? " first" : ""}${dark ? " dark" : ""}`);
        key.type = "button";
        key.dataset.degree = String(degree);
        key.append(el("strong", "", nameOf(degree)), el("small", "", hz(frequencyOf(degree))));
        const hint = hints.get(degree);
        if (hint) key.appendChild(el("kbd", "", hint));
        key.addEventListener("pointerdown", (event) => {
            event.preventDefault();
            key.setPointerCapture?.(event.pointerId);
            noteOn(`pointer:${event.pointerId}`, degree, 1);
        });
        const release = (event) => noteOff(`pointer:${event.pointerId}`);
        key.addEventListener("pointerup", release);
        key.addEventListener("pointercancel", release);
        key.addEventListener("lostpointercapture", release);
        fragment.appendChild(key);
    }
    keysNode.appendChild(fragment);
}

function mark(degree, down) {
    for (const node of keysNode.querySelectorAll(`[data-degree="${degree}"]`)) {
        node.classList.toggle("down", down);
    }
    for (const row of degreesBody.querySelectorAll("tr")) {
        const count = steps();
        const rowDegree = Number(row.dataset.degree);
        if (rowDegree === degree % count || (rowDegree === count && degree % count === 0 && degree > 0)) {
            row.classList.toggle("playing", down);
        }
    }
}

// ---------------------------------------------------------------- audio

async function ensureAudio() {
    const Context = window.AudioContext || window.webkitAudioContext;
    if (!Context) {
        fail("Audio is not available in this browser.");
        return false;
    }
    audio ||= new Context();
    if (audio.state === "suspended") await audio.resume();
    return true;
}

function gainLevel(velocity = 1) {
    return Math.max(0.0001, clamp(volume.value, 0, 1, 0.3) * Math.max(0.05, velocity));
}

async function noteOn(id, degree, velocity = 1) {
    if (!current || voices.has(id)) return;
    if (!(await ensureAudio())) return;
    const frequency = frequencyOf(degree);
    if (!(frequency > 0)) return;
    const oscillator = audio.createOscillator();
    const gain = audio.createGain();
    const now = audio.currentTime;
    oscillator.type = waveform.value;
    oscillator.frequency.setValueAtTime(frequency, now);
    gain.gain.setValueAtTime(0.0001, now);
    gain.gain.exponentialRampToValueAtTime(gainLevel(velocity), now + 0.012);
    oscillator.connect(gain).connect(audio.destination);
    oscillator.start(now);
    voices.set(id, { oscillator, gain, degree });
    mark(degree, true);
    if (current.adaptive) retune();
}

function noteOff(id) {
    const voice = voices.get(id);
    if (!voice) return;
    voices.delete(id);
    const now = audio.currentTime;
    voice.gain.gain.cancelScheduledValues(now);
    voice.gain.gain.setValueAtTime(Math.max(0.0001, voice.gain.gain.value), now);
    voice.gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.12);
    voice.oscillator.stop(now + 0.16);
    if (![...voices.values()].some((other) => other.degree === voice.degree)) mark(voice.degree, false);
    if (current?.adaptive) retune();
}

/// In the adaptive tuning the lowest held key is the root, so the others
/// move when it changes.
function retune() {
    if (!audio) return;
    const now = audio.currentTime;
    for (const voice of voices.values()) {
        const frequency = frequencyOf(voice.degree);
        if (frequency > 0) voice.oscillator.frequency.setTargetAtTime(frequency, now, 0.01);
    }
}

function releaseAll() {
    for (const id of [...voices.keys()]) noteOff(id);
    for (const node of document.querySelectorAll(".key.down, tr.playing")) {
        node.classList.remove("down", "playing");
    }
}

async function playOnce(degree) {
    if (!(await ensureAudio())) return;
    stopSequence();
    playAt(degree, audio.currentTime, 0.5);
}

function playAt(degree, at, duration) {
    const oscillator = audio.createOscillator();
    const gain = audio.createGain();
    oscillator.type = waveform.value;
    oscillator.frequency.setValueAtTime(frequencyOf(degree), at);
    gain.gain.setValueAtTime(0.0001, at);
    gain.gain.exponentialRampToValueAtTime(gainLevel(), at + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + duration);
    oscillator.connect(gain).connect(audio.destination);
    oscillator.start(at);
    oscillator.stop(at + duration + 0.05);
    sequence.push({ oscillator, timer: null });
    const timer = window.setTimeout(() => mark(degree, true), Math.max(0, (at - audio.currentTime) * 1000));
    const off = window.setTimeout(() => mark(degree, false), Math.max(0, (at - audio.currentTime + duration) * 1000));
    sequence.push({ oscillator: null, timer }, { oscillator: null, timer: off });
}

async function playScale() {
    if (!current || !(await ensureAudio())) return;
    stopSequence();
    const count = steps();
    const degrees = twelveTone() ? majorScaleSemitones : [...Array(count + 1).keys()];
    const step = 0.36;
    const start = audio.currentTime + 0.03;
    degrees.forEach((degree, index) => playAt(degree, start + index * step, step * 0.9));
}

function stopSequence() {
    for (const entry of sequence) {
        if (entry.timer !== null) window.clearTimeout(entry.timer);
        try {
            entry.oscillator?.stop();
        } catch {
            // Already stopped.
        }
    }
    sequence = [];
    for (const node of degreesBody.querySelectorAll("tr.playing")) node.classList.remove("playing");
    for (const node of keysNode.querySelectorAll(".key.down")) node.classList.remove("down");
}

// ---------------------------------------------------------------- computer keyboard

function typing() {
    const active = document.activeElement;
    if (!active) return false;
    return ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName) || active.isContentEditable;
}

window.addEventListener("keydown", (event) => {
    if (event.repeat || typing() || event.metaKey || event.ctrlKey || event.altKey) return;
    const degree = keymap().get(event.code);
    if (degree === undefined) return;
    event.preventDefault();
    noteOn(`key:${event.code}`, degree, 1);
});

window.addEventListener("keyup", (event) => noteOff(`key:${event.code}`));
window.addEventListener("blur", releaseAll);
document.addEventListener("visibilitychange", () => {
    if (document.hidden) releaseAll();
});

// ---------------------------------------------------------------- MIDI

function onMidiMessage(event) {
    const data = event.data;
    if (!data || data.length < 3) return;
    const status = data[0] & 0xf0;
    const note = data[1];
    const velocity = data[2];
    if (status === 0x90 && velocity > 0) {
        noteOn(`midi:${note}`, note - 60, velocity / 127);
    } else if (status === 0x80 || (status === 0x90 && velocity === 0)) {
        noteOff(`midi:${note}`);
    } else if (status === 0xb0 && data[1] === 123) {
        releaseAll();
    }
}

function wireMidi() {
    if (!midiAccess) return;
    let count = 0;
    for (const input of midiAccess.inputs.values()) {
        input.onmidimessage = onMidiMessage;
        count += 1;
    }
    statusNode.textContent = count
        ? `MIDI: ${count} input${count === 1 ? "" : "s"} connected. Middle C is the root; each key up is one degree.`
        : "MIDI: no input devices found. Plug one in and it will be picked up.";
    midiButton.classList.toggle("on", count > 0);
}

midiButton.addEventListener("click", async () => {
    if (!navigator.requestMIDIAccess) {
        statusNode.textContent = "MIDI: this browser has no Web MIDI. Chrome, Edge and Opera do.";
        return;
    }
    try {
        midiAccess ||= await navigator.requestMIDIAccess({ sysex: false });
        midiAccess.onstatechange = wireMidi;
        wireMidi();
    } catch (err) {
        statusNode.textContent = `MIDI: ${err instanceof Error ? err.message : String(err)}`;
    }
});

// ---------------------------------------------------------------- URL

function syncUrl() {
    const url = new URL(window.location.href);
    if (current && !current.id.startsWith("pasted:")) url.searchParams.set("system", current.id);
    else url.searchParams.delete("system");
    url.searchParams.set("root", String(rootHz));
    if (octaves() !== 0) url.searchParams.set("octave", String(octaves()));
    else url.searchParams.delete("octave");
    window.history.replaceState({}, "", url.href);
    return url.href;
}

function readUrl() {
    const params = new URLSearchParams(window.location.search);
    selectedId = params.get("system") ?? "";
    rootHz = clamp(params.get("root"), 20, 2000, defaultRootFrequency);
    transposeInput.value = String(Math.round(clamp(params.get("octave"), -4, 4, 0)));
}

shareButton.addEventListener("click", async () => {
    const href = syncUrl();
    try {
        if (navigator.clipboard?.writeText && window.isSecureContext) {
            await navigator.clipboard.writeText(href);
        } else {
            const textarea = el("textarea");
            textarea.value = href;
            textarea.style.position = "fixed";
            textarea.style.left = "-9999px";
            document.body.appendChild(textarea);
            textarea.select();
            document.execCommand("copy");
            textarea.remove();
        }
        shareButton.textContent = "Copied";
        shareButton.classList.add("copied");
        if (shareTimer) clearTimeout(shareTimer);
        shareTimer = setTimeout(() => {
            shareButton.textContent = "Copy link";
            shareButton.classList.remove("copied");
        }, 1600);
    } catch {
        fail("The URL has been updated in your address bar.");
    }
});

// ---------------------------------------------------------------- wiring

chips.addEventListener("click", (event) => {
    const chip = event.target.closest(".chip");
    if (!chip) return;
    kind = chip.dataset.kind;
    for (const node of chips.querySelectorAll(".chip")) node.classList.toggle("active", node === chip);
    renderPicker();
});

let searchTimer = null;
searchInput.addEventListener("input", () => {
    if (searchTimer !== null) window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
        searchTimer = null;
        renderPicker();
    }, 120);
});

browseButton.addEventListener("click", () => {
    document.body.classList.toggle("browsing");
    if (document.body.classList.contains("browsing")) searchInput.focus();
});

layoutSelect.addEventListener("change", () => {
    releaseAll();
    renderKeys();
});

transposeInput.addEventListener("change", () => {
    releaseAll();
    renderKeys();
    renderDegrees();
    syncUrl();
});

playScaleButton.addEventListener("click", playScale);
stopButton.addEventListener("click", () => {
    stopSequence();
    releaseAll();
});

window.addEventListener("popstate", () => {
    readUrl();
    stopSequence();
    releaseAll();
    start();
});

readUrl();
start();
