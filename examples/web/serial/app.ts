// @ts-nocheck
import "../theme.js";
import { midiToHz, play, stop } from "../play.js";

const $ = (selector) => document.querySelector(selector);
const form = $("#form");
const rowInput = $("#row");
const historicalSelect = $("#historical");
const playButton = $("#play");
const errorNode = $("#error");
const title = $("#title");
const facts = $("#facts");
const pitches = $("#pitches");
const matrix = $("#matrix");
const formsTitle = $("#forms-title");
const forms = $("#forms");

$("#docs-link").href = "../docs/music21_rs/index.html";

let wasm = null;
let current = null;
let historical = [];

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

function playClasses(classes) {
    stop();
    // Up from C4, each pitch class in the octave above middle C.
    return play(classes.map((pc) => midiToHz(60 + pc)), { arpeggio: true, step: 0.28 });
}

function render(info) {
    current = info;
    const match = info.historical[0];
    title.textContent = match
        ? `${match.composer}, ${match.title}${match.opus ? ` (${match.opus})` : ""}`
        : info.is_twelve_tone_row
          ? "Twelve-tone row"
          : `${info.pitch_classes.length}-note row`;
    facts.replaceChildren(
        fact("Pitch classes", info.pitch_classes.join(" ")),
        fact("Notes", info.names.join(" ")),
        fact("Twelve-tone row", info.is_twelve_tone_row ? "yes" : "no"),
        fact("Intervals", info.intervals),
        fact("All-interval", info.is_all_interval === null ? "n/a" : info.is_all_interval ? "yes" : "no"),
        fact("Link chord", info.link ? `no. ${info.link.number}, ${info.link.special_intervals.join(" ")}` : "no"),
        fact("Historical", info.historical.length ? info.historical.map((row) => row.name).join(", ") : "none"),
    );
    pitches.replaceChildren();
    info.names.forEach((name, index) => {
        const node = el("div", "pitch");
        node.append(el("strong", "", name), el("small", "", String(info.pitch_classes[index])));
        pitches.appendChild(node);
    });

    matrix.replaceChildren();
    if (info.matrix.length) {
        const size = info.matrix.length;
        matrix.style.gridTemplateColumns = `repeat(${size + 1}, minmax(30px, 1fr))`;
        matrix.appendChild(el("span", "cell head", ""));
        info.matrix[0].forEach((pc) => matrix.appendChild(el("span", "cell head", `I${pc}`)));
        info.matrix.forEach((row, rowIndex) => {
            matrix.appendChild(el("span", "cell head", `P${row[0]}`));
            row.forEach((pc, column) => {
                matrix.appendChild(el("span", `cell${rowIndex === column ? " diagonal" : ""}`, String(pc)));
            });
        });
    } else {
        matrix.appendChild(el("div", "about", "A matrix needs all twelve pitch classes once each."));
    }

    forms.replaceChildren();
    formsTitle.textContent = info.forms.length ? `Forms (${info.forms.length})` : "Forms";
    for (const row of info.forms) {
        const tr = el("tr");
        tr.append(el("td", "", row.label), el("td", "", row.pitch_classes.join(" ")), el("td", "", row.names.join(" ")));
        const cell = el("td");
        const button = el("button", "secondary", "Play");
        button.type = "button";
        button.style.height = "30px";
        button.style.padding = "0 10px";
        button.addEventListener("click", () => playClasses(row.pitch_classes));
        cell.appendChild(button);
        tr.appendChild(cell);
        forms.appendChild(tr);
    }
}

function read() {
    if (!wasm) return;
    try {
        render(wasm.tone_row(rowInput.value.trim()));
        clearError();
        const url = new URL(window.location.href);
        url.searchParams.set("row", rowInput.value.trim());
        window.history.replaceState({}, "", url.href);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

form.addEventListener("submit", (event) => {
    event.preventDefault();
    historicalSelect.value = "";
    read();
});

historicalSelect.addEventListener("change", () => {
    const row = historical.find((entry) => entry.name === historicalSelect.value);
    if (!row) return;
    rowInput.value = row.pitch_classes.join(" ");
    read();
});

playButton.addEventListener("click", async () => {
    if (!current) return;
    if (!(await playClasses(current.pitch_classes))) fail("Audio is not available in this browser.");
});

async function start() {
    try {
        const module = await import(new URL("../pkg/music21_rs_web.js", window.location.href).href);
        await module.default();
        wasm = module;
        historical = wasm.historical_rows();
        for (const row of historical) {
            const option = el("option", "", `${row.composer}: ${row.title}${row.opus ? ` (${row.opus})` : ""}`);
            option.value = row.name;
            historicalSelect.appendChild(option);
        }
        const shared = new URLSearchParams(window.location.search).get("row");
        if (shared) rowInput.value = shared;
        read();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

start();
