// @ts-nocheck
import "../theme.js";
import { play, stop } from "../play.js";
import { el, errorLine, fact } from "../dom.js";

const $ = (selector) => document.querySelector(selector);
const form = $("#form");
const labelInput = $("#label");
const playButton = $("#play");
const examples = $("#examples");
const errorNode = $("#error");
const { fail, clearError } = errorLine(errorNode);
const title = $("#title");
const facts = $("#facts");
const pitches = $("#pitches");
const actions = $("#actions");
const hot = $("#hot");
const hotRoot = $("#hot-root");
const shorthands = $("#shorthands");

$("#docs-link").href = "../docs/music21_rs/index.html";

const exampleLabels = [
    "C:maj",
    "A:min",
    "C:maj7/3",
    "Bb:min7/b3",
    "F:maj7(#11)",
    "C:sus4(*3,9)",
    "Ab:maj(9)/9",
    "E:(b3,5,b7,11)",
    "D:hdim7",
    "G:7(b9,#11)",
    "N",
];

let wasm = null;
let current = null;

function bits(target, values, from) {
    target.replaceChildren();
    const names = from === "C" ? wasm.display_pitch_class_names() : null;
    values.forEach((value, index) => {
        const cell = el("span", value ? "on" : "", names ? names[index] : String(index));
        target.appendChild(cell);
    });
}

function render(info) {
    current = info;
    title.textContent = info.is_empty ? "No chord" : info.pretty;
    facts.replaceChildren(
        fact("As written", info.label),
        fact("Canonical", info.canonical),
        fact("Prettified", info.pretty),
        fact("Root", info.root ?? "none"),
        fact("Bass degree", info.bass ?? "none"),
        fact("Shorthand", info.shorthand ?? "none"),
        fact("Written degrees", info.written_degrees.length ? info.written_degrees.join(", ") : "none"),
        fact("Sounding degrees", info.sounding_degrees.length ? info.sounding_degrees.join(", ") : "none"),
        fact("Common name", info.common_name),
        fact("Pitched name", info.pitched_common_name),
        fact("Forte class", info.forte_class ?? "none"),
        fact("Chord symbol", info.chord_symbol ?? "none"),
    );
    pitches.replaceChildren();
    info.pitches.forEach((pitch, index) => {
        const node = el("div", `pitch${index === info.bass_index ? " bass" : ""}`);
        node.append(el("strong", "", pitch), el("small", "", `MIDI ${info.midi[index] ?? ""}`));
        pitches.appendChild(node);
    });
    actions.replaceChildren();
    if (info.pitches.length) {
        const inspector = el("a", "", "Open in Chord Inspector");
        inspector.href = `../chord/?chord=${encodeURIComponent(info.pitches.join(" "))}`;
        actions.appendChild(inspector);
    }
    bits(hot, info.multi_hot, "C");
    bits(hotRoot, info.multi_hot_from_root, "root");
}

function read(label) {
    if (!wasm) return;
    try {
        render(wasm.harte_chord(label));
        clearError();
        const url = new URL(window.location.href);
        url.searchParams.set("label", label);
        window.history.replaceState({}, "", url.href);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

async function start() {
    try {
        const module = await import(new URL("../pkg/music21_rs_web.js", window.location.href).href);
        await module.default();
        wasm = module;
        for (const entry of wasm.harte_shorthands()) {
            const row = el("tr");
            row.append(el("td", "", entry.name), el("td", "", entry.degrees.join(" ")));
            row.style.cursor = "pointer";
            row.addEventListener("click", () => {
                labelInput.value = `C:${entry.name}`;
                read(labelInput.value);
            });
            shorthands.appendChild(row);
        }
        const shared = new URLSearchParams(window.location.search).get("label");
        if (shared) labelInput.value = shared;
        read(labelInput.value);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

for (const label of exampleLabels) {
    const chip = el("button", "chip", label);
    chip.type = "button";
    chip.addEventListener("click", () => {
        labelInput.value = label;
        read(label);
    });
    examples.appendChild(chip);
}

form.addEventListener("submit", (event) => {
    event.preventDefault();
    read(labelInput.value.trim());
});

playButton.addEventListener("click", async () => {
    if (!current || !current.midi.length) return;
    stop();
    if (!(await play(current.frequencies_hz))) fail("Audio is not available in this browser.");
});

start();
