// @ts-nocheck
import "../theme.js";
import { midiToHz, play, stop } from "../play.js";

const $ = (selector) => document.querySelector(selector);
const form = $("#form");
const typeSelect = $("#type");
const tonicInput = $("#tonic");
const playButton = $("#play");
const errorNode = $("#error");
const title = $("#title");
const facts = $("#facts");
const pitches = $("#pitches");
const actions = $("#actions");
const deriveForm = $("#derive-form");
const deriveInput = $("#derive-input");
const derived = $("#derived");

$("#docs-link").href = "../docs/music21_rs/index.html";

let wasm = null;
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

function render(info) {
    current = info;
    title.textContent = `${info.tonic.replace(/\d+$/, "")} ${info.name}`;
    facts.replaceChildren(
        fact("music21 class", info.id),
        fact("Tonic", info.tonic),
        fact("Notes per octave", String(info.pitch_names.length - 1)),
        fact("Steps", info.steps.join(" ")),
    );
    if (info.degrees.length) facts.appendChild(fact("Degrees", info.degrees.join(" ")));
    pitches.replaceChildren();
    info.pitches.forEach((pitch, index) => {
        const node = el("div", `pitch${index === 0 || index === info.pitches.length - 1 ? " tonic" : ""}`);
        const degree = info.degrees.length ? info.degrees[index % info.degrees.length] : index + 1;
        node.append(el("strong", "", pitch), el("small", "", `degree ${index === info.pitches.length - 1 ? info.degrees[0] ?? 1 : degree}`));
        pitches.appendChild(node);
    });
    actions.replaceChildren();
    const inspector = el("a", "", "Open as a chord");
    inspector.href = `../chord/?chord=${encodeURIComponent(info.pitches.slice(0, -1).join(" "))}`;
    actions.appendChild(inspector);
    const tuning = el("a", "", "Hear it in other tunings");
    tuning.href = "../tuning/";
    actions.appendChild(tuning);
}

function realize() {
    if (!wasm) return;
    try {
        render(wasm.realize_scale(typeSelect.value, tonicInput.value.trim()));
        clearError();
        const url = new URL(window.location.href);
        url.searchParams.set("type", typeSelect.value);
        url.searchParams.set("tonic", tonicInput.value.trim());
        window.history.replaceState({}, "", url.href);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

function derive() {
    if (!wasm) return;
    try {
        const found = wasm.derive_scales(deriveInput.value.trim(), 24);
        derived.replaceChildren();
        for (const entry of found) {
            const row = el("tr", "clickable");
            row.append(
                el("td", "", `${entry.matched} of ${entry.total}`),
                el("td", "", `${entry.scale.tonic.replace(/\d+$/, "")} ${entry.scale.name}`),
                el("td", "", entry.scale.pitch_names.slice(0, -1).join(" ")),
            );
            row.addEventListener("click", () => {
                typeSelect.value = entry.scale.id;
                tonicInput.value = entry.scale.tonic;
                realize();
            });
            derived.appendChild(row);
        }
        clearError();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

form.addEventListener("submit", (event) => {
    event.preventDefault();
    realize();
});

typeSelect.addEventListener("change", realize);

deriveForm.addEventListener("submit", (event) => {
    event.preventDefault();
    derive();
});

playButton.addEventListener("click", async () => {
    if (!current) return;
    stop();
    const midi = current.pitches.map((name) => wasm.pitch_midi_number(name));
    if (!(await play(midi.map(midiToHz), { arpeggio: true, step: 0.32 }))) {
        fail("Audio is not available in this browser.");
    }
});

async function start() {
    try {
        const module = await import(new URL("../pkg/music21_rs_web.js", window.location.href).href);
        await module.default();
        wasm = module;
        for (const entry of wasm.scale_types()) {
            const option = el("option", "", `${entry.name} (${entry.steps})`);
            option.value = entry.id;
            typeSelect.appendChild(option);
        }
        const params = new URLSearchParams(window.location.search);
        if (params.has("type")) typeSelect.value = params.get("type") ?? "";
        if (!typeSelect.value) typeSelect.value = "MajorScale";
        if (params.has("tonic")) tonicInput.value = params.get("tonic") ?? "";
        realize();
        derive();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

start();
