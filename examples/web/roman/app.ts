// @ts-nocheck
import "../theme.js";
import { midiToHz, play, stop } from "../play.js";

const $ = (selector) => document.querySelector(selector);
const modes = $("#modes");
const form = $("#form");
const input = $("#input");
const inputLabel = $("#input-label");
const keyInput = $("#key");
const playButton = $("#play");
const examples = $("#examples");
const errorNode = $("#error");
const title = $("#title");
const facts = $("#facts");
const pitches = $("#pitches");
const actions = $("#actions");

$("#docs-link").href = "../docs/music21_rs/index.html";

const realizeExamples = [
    ["I", "C"],
    ["V7", "C"],
    ["ii65", "C"],
    ["viio7", "a"],
    ["V65/V", "g"],
    ["It6", "c"],
    ["Ger65", "c"],
    ["Fr43", "c"],
    ["N6", "e"],
    ["Cad64", "D"],
    ["bVII", "E"],
    ["V7[add4]", "F"],
    ["I[no3]", "G"],
];
const nameExamples = [
    ["G B D F", "C"],
    ["A- C E- F#", "c"],
    ["C E- G-", "C"],
    ["D F# A C", "G"],
    ["F A C", "a"],
    ["B D F A-", "c"],
    ["E G# B D", ""],
    ["D- F A-", "C"],
];

let wasm = null;
let mode = "realize";
let current = null;

function fail(message) {
    errorNode.textContent = message;
    errorNode.style.display = "block";
}

function clearError() {
    errorNode.style.display = "none";
}

function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
}

function fact(label, value) {
    const node = el("div", "fact");
    node.append(el("span", "", label), el("strong", "", value));
    return node;
}

function renderExamples() {
    examples.replaceChildren();
    for (const [value, key] of mode === "realize" ? realizeExamples : nameExamples) {
        const chip = el("button", "chip", key ? `${value} in ${key}` : value);
        chip.type = "button";
        chip.addEventListener("click", () => {
            input.value = value;
            keyInput.value = key;
            run();
        });
        examples.appendChild(chip);
    }
}

function render(info) {
    current = info;
    title.textContent = `${info.figure} in ${info.key}`;
    const degree = `${info.degree}${info.alteration > 0 ? " raised" : info.alteration < 0 ? " lowered" : ""}`;
    facts.replaceChildren(
        fact("Figure", info.figure),
        fact("Numeral", info.roman_numeral),
        fact("Key", info.key),
        fact("Scale degree", degree),
        fact("Inversion", String(info.inversion)),
        fact("Implied quality", info.quality),
        fact("Common name", info.common_name),
        fact("Pitched name", info.pitched_common_name),
        fact("Functionality score", String(info.functionality_score)),
        fact("Neapolitan", info.is_neapolitan ? "yes" : "no"),
        fact("Mixture", info.is_mixture ? "yes" : "no"),
    );
    pitches.replaceChildren();
    info.pitches.forEach((pitch, index) => {
        const node = el("div", `pitch${index === 0 ? " bass" : ""}`);
        node.append(el("strong", "", pitch), el("small", "", index === 0 ? "bass" : info.pitch_names[index]));
        pitches.appendChild(node);
    });
    actions.replaceChildren();
    const inspector = el("a", "", "Open in Chord Inspector");
    inspector.href = `../chord/?chord=${encodeURIComponent(info.pitches.join(" "))}&key=${encodeURIComponent(info.key.split(" ")[0])}`;
    actions.appendChild(inspector);
}

function run() {
    if (!wasm) return;
    const value = input.value.trim();
    const key = keyInput.value.trim();
    try {
        const info = mode === "realize" ? wasm.realize_roman(value, key || "C") : wasm.roman_from_chord(value, key);
        render(info);
        clearError();
        const url = new URL(window.location.href);
        url.searchParams.set("mode", mode);
        url.searchParams.set("input", value);
        url.searchParams.set("key", key);
        window.history.replaceState({}, "", url.href);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

function setMode(next) {
    mode = next;
    for (const chip of modes.querySelectorAll(".chip")) chip.classList.toggle("active", chip.dataset.mode === mode);
    inputLabel.textContent = mode === "realize" ? "Figure" : "Chord";
    input.placeholder = mode === "realize" ? "V7, ii65, viio7/V, Ger65" : "G B D F, or MIDI 60 64 67";
    renderExamples();
}

modes.addEventListener("click", (event) => {
    const chip = event.target.closest(".chip");
    if (!chip) return;
    setMode(chip.dataset.mode);
    const [value, key] = (mode === "realize" ? realizeExamples : nameExamples)[0];
    input.value = value;
    keyInput.value = key;
    run();
});

form.addEventListener("submit", (event) => {
    event.preventDefault();
    run();
});

playButton.addEventListener("click", async () => {
    if (!current) return;
    stop();
    const midi = current.pitches.map((name) => wasm.pitch_midi_number(name));
    if (!(await play(midi.map(midiToHz)))) fail("Audio is not available in this browser.");
});

async function start() {
    try {
        const module = await import(new URL("../pkg/music21_rs_web.js", window.location.href).href);
        await module.default();
        wasm = module;
        const params = new URLSearchParams(window.location.search);
        setMode(params.get("mode") === "name" ? "name" : "realize");
        if (params.has("input")) input.value = params.get("input") ?? "";
        if (params.has("key")) keyInput.value = params.get("key") ?? "";
        run();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

start();
