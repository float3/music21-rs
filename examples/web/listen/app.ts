import "../theme.js";

type HeardNote = {
    midi: number;
    name: string;
    frequency_hz: number;
    cents: number;
    strength: number;
    sounding: boolean;
};

type HeardChord = {
    midi: number[];
    pitch_names: string[];
    pitch_classes: number[];
    common_name: string;
    pitched_common_name: string;
    chord_symbol: string | null;
    root: string | null;
    bass: string | null;
    inversion_name: string | null;
};

type Frame = {
    level_db: number;
    notes: HeardNote[];
    chord: HeardChord | null;
};

type Listener = {
    set_sensitivity(sensitivity: number): void;
    set_gate_db(gateDb: number): void;
    set_hold_ms(holdMs: number): void;
    clear(): void;
    listen(samples: Float32Array, timeMs: number): Frame;
};

type WasmModule = {
    default: () => Promise<unknown>;
    ChordListener: new (sampleRate: number) => Listener;
};

type Mode = "off" | "mic" | "demo" | "file";

type DemoStep = {
    label: string;
    midi: number[];
    arpeggio: boolean;
};

const lowestMidi = 36;
const highestMidi = 96;
const pitchClassNames = ["C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
const blackPitchClasses = new Set([1, 3, 6, 8, 10]);
const demoSteps: DemoStep[] = [
    { label: "C major, together", midi: [48, 55, 60, 64], arpeggio: false },
    { label: "A minor, arpeggiated", midi: [45, 52, 57, 60, 64], arpeggio: true },
    { label: "F major seventh, together", midi: [41, 48, 57, 64], arpeggio: false },
    { label: "G dominant seventh, arpeggiated", midi: [43, 59, 62, 65], arpeggio: true },
    { label: "E major in first inversion, together", midi: [56, 59, 64, 68], arpeggio: false },
    { label: "D half-diminished seventh, arpeggiated", midi: [50, 53, 56, 60], arpeggio: true },
];
const demoStepSeconds = 3;
const demoArpeggioSeconds = 0.32;
const demoRestSeconds = 0.6;
const historyStableMs = 300;
const historyLength = 30;
const historyGrowMs = 1500;
const analysisIntervalMs = 16;

function element<T extends HTMLElement>(selector: string): T {
    const node = document.querySelector<T>(selector);
    if (!node) throw new Error(`The page is missing ${selector}.`);
    return node;
}

const micButton = element<HTMLButtonElement>("#mic");
const demoButton = element<HTMLButtonElement>("#demo");
const fileInput = element<HTMLInputElement>("#file");
const clearButton = element<HTMLButtonElement>("#clear");
const deviceField = element<HTMLLabelElement>("#device-field");
const deviceSelect = element<HTMLSelectElement>("#device");
const holdInput = element<HTMLInputElement>("#hold");
const holdValue = element<HTMLOutputElement>("#hold-value");
const sensitivityInput = element<HTMLInputElement>("#sensitivity");
const sensitivityValue = element<HTMLOutputElement>("#sensitivity-value");
const gateInput = element<HTMLInputElement>("#gate");
const gateValue = element<HTMLOutputElement>("#gate-value");
const sizeSelect = element<HTMLSelectElement>("#size");
const errorNode = element<HTMLParagraphElement>("#error");
const player = element<HTMLAudioElement>("#player");
const stage = element<HTMLElement>("#stage");
const statusNode = element<HTMLDivElement>("#status");
const symbolNode = element<HTMLDivElement>("#symbol");
const chordNameNode = element<HTMLDivElement>("#chord-name");
const meterFill = element<HTMLSpanElement>("#meter-fill");
const meterGate = element<HTMLSpanElement>("#meter-gate");
const keyboard = element<HTMLDivElement>("#keyboard");
const facts = element<HTMLDivElement>("#facts");
const pitches = element<HTMLDivElement>("#pitches");
const actions = element<HTMLDivElement>("#actions");
const hot = element<HTMLDivElement>("#hot");
const notesBody = element<HTMLTableSectionElement>("#notes");
const spectrum = element<HTMLCanvasElement>("#spectrum");
const historyNode = element<HTMLDivElement>("#history");

let wasm: WasmModule | null = null;
let audio: AudioContext | null = null;
let analyser: AnalyserNode | null = null;
let listener: Listener | null = null;
let timeData = new Float32Array(0);
let frequencyData = new Float32Array(0);
let mode: Mode = "off";
let frameRequest = 0;
let analysisTimer = 0;
let lastFrame: Frame | null = null;
let stream: MediaStream | null = null;
let micSource: MediaStreamAudioSourceNode | null = null;
let fileSource: MediaElementAudioSourceNode | null = null;
let fileName = "";
let demo: {
    output: GainNode;
    timer: number;
    oscillators: OscillatorNode[];
    starts: { at: number; label: string }[];
} | null = null;

const keys = new Map<number, HTMLDivElement>();
const hotCells: HTMLSpanElement[] = [];
let shownNotes = "";
let shownChord = "";
let chordSince = 0;
let tableShownAt = 0;
let history: HeardChord[] = [];
let rememberedAt = 0;
let colours = readColours();
let coloursReadAt = 0;

function fail(message: string): void {
    errorNode.textContent = message;
    errorNode.style.display = "block";
}

function clearError(): void {
    errorNode.style.display = "none";
}

function make<K extends keyof HTMLElementTagNameMap>(
    tag: K,
    className = "",
    text?: string,
): HTMLElementTagNameMap[K] {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
}

function midiToHz(midi: number): number {
    return 440 * Math.pow(2, (midi - 69) / 12);
}

function noteName(midi: number): string {
    return `${pitchClassNames[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;
}

function displayFlats(text: string): string {
    return text.replace(/([A-G])-/g, "$1b").replace(/([A-G]b)-/g, "$1b");
}

function readColours() {
    const style = getComputedStyle(document.documentElement);
    const token = (name: string, fallback: string) =>
        style.getPropertyValue(name).trim() || fallback;
    return {
        ink: token("--ink", "#151515"),
        muted: token("--muted", "#62666f"),
        line: token("--line", "#d9dde5"),
        accent: token("--accent", "#0f766e"),
        accentLine: token("--accent-line", "#b7d8d4"),
    };
}

function buildKeyboard(): void {
    const whites: number[] = [];
    for (let midi = lowestMidi; midi <= highestMidi; midi += 1) {
        if (!blackPitchClasses.has(midi % 12)) whites.push(midi);
    }
    const whiteWidth = 100 / whites.length;
    let whiteIndex = 0;
    for (let midi = lowestMidi; midi <= highestMidi; midi += 1) {
        const black = blackPitchClasses.has(midi % 12);
        const key = make("div", `key ${black ? "black" : "white"}`);
        key.title = noteName(midi);
        if (black) {
            const width = whiteWidth * 0.62;
            key.style.left = `${whiteIndex * whiteWidth - width / 2}%`;
            key.style.width = `${width}%`;
        } else {
            key.style.left = `${whiteIndex * whiteWidth}%`;
            key.style.width = `${whiteWidth}%`;
            whiteIndex += 1;
            if (midi % 12 === 0) key.appendChild(make("small", "", noteName(midi)));
        }
        keys.set(midi, key);
        keyboard.appendChild(key);
    }
    for (const name of pitchClassNames) {
        const cell = make("span", "", name);
        hotCells.push(cell);
        hot.appendChild(cell);
    }
}

function fact(label: string, value: string): HTMLDivElement {
    const node = make("div", "fact");
    node.append(make("span", "", label), make("strong", "", value));
    return node;
}

function renderNotes(notes: HeardNote[]): void {
    for (const key of keys.values()) key.classList.remove("held", "sounding");
    for (const cell of hotCells) cell.classList.remove("on", "sounding");
    for (const note of notes) {
        const key = keys.get(note.midi);
        key?.classList.add(note.sounding ? "sounding" : "held");
        if (key) key.title = `${note.name}, ${formatCents(note.cents)}`;
        const cell = hotCells[((note.midi % 12) + 12) % 12];
        cell.classList.add("on");
        if (note.sounding) cell.classList.add("sounding");
    }
}

function formatCents(cents: number): string {
    const rounded = Math.round(cents);
    return `${rounded > 0 ? "+" : ""}${rounded} cents`;
}

function renderTable(notes: HeardNote[]): void {
    notesBody.replaceChildren(
        ...notes.map((note) => {
            const row = make("tr", note.sounding ? "" : "released");
            const cents = Math.round(note.cents);
            row.append(
                make("td", "", note.name),
                make("td", "number", note.frequency_hz.toFixed(1)),
                make("td", "number", `${cents > 0 ? "+" : ""}${cents}`),
                make("td", "number", `${Math.round(note.strength * 100)}%`),
            );
            return row;
        }),
    );
}

function inspectorLink(chord: HeardChord): string {
    return `../chord/?chord=${encodeURIComponent(chord.midi.join(" "))}`;
}

function chordTitle(chord: HeardChord): string {
    return displayFlats(chord.chord_symbol || chord.pitched_common_name);
}

function renderChord(chord: HeardChord | null): void {
    stage.classList.toggle("stale", !chord);
    if (!chord) return;
    symbolNode.textContent = chordTitle(chord);
    chordNameNode.textContent = [chord.pitched_common_name, chord.inversion_name]
        .filter(Boolean)
        .join(", ");
    facts.replaceChildren(
        fact("Symbol", chord.chord_symbol ? displayFlats(chord.chord_symbol) : "none"),
        fact("Common name", chord.common_name),
        fact("Root", chord.root ?? "none"),
        fact("Bass", chord.bass ?? "none"),
        fact("Inversion", chord.inversion_name ?? "none"),
        fact("Pitch classes", chord.pitch_classes.map((pc) => pitchClassNames[pc]).join(" ")),
    );
    pitches.replaceChildren(
        ...chord.midi.map((midi, index) => {
            const node = make("div", `pitch${index === 0 ? " bass" : ""}`);
            node.append(make("strong", "", chord.pitch_names[index]), make("small", "", `MIDI ${midi}`));
            return node;
        }),
    );
    const inspector = make("a", "", "Open in Chord Inspector");
    inspector.href = inspectorLink(chord);
    actions.replaceChildren(inspector);
}

function remember(chord: HeardChord, now: number): void {
    if (chord.pitch_classes.length < 2) return;
    const last = history[0];
    if (last && last.pitched_common_name === chord.pitched_common_name && last.bass === chord.bass) return;
    const grows =
        last !== undefined &&
        now - rememberedAt < historyGrowMs &&
        last.bass === chord.bass &&
        last.pitch_classes.every((pitchClass) => chord.pitch_classes.includes(pitchClass));
    history = [chord, ...(grows ? history.slice(1) : history)].slice(0, historyLength);
    rememberedAt = now;
    historyNode.replaceChildren(
        ...history.map((entry) => {
            const chip = make("a", "chip", chordTitle(entry));
            chip.href = inspectorLink(entry);
            chip.title = `${entry.pitched_common_name}: ${entry.pitch_names.join(" ")}`;
            return chip;
        }),
    );
}

function render(frame: Frame, now: number): void {
    const level = Math.max(0, Math.min(1, (frame.level_db + 90) / 90));
    meterFill.style.height = `${level * 100}%`;

    const notesKey = frame.notes.map((note) => `${note.midi}${note.sounding ? "*" : ""}`).join(" ");
    if (notesKey !== shownNotes) {
        shownNotes = notesKey;
        renderNotes(frame.notes);
        renderTable(frame.notes);
        tableShownAt = now;
    } else if (now - tableShownAt > 150) {
        renderTable(frame.notes);
        tableShownAt = now;
    }

    const chordKey = frame.chord ? frame.chord.midi.join(" ") : "";
    if (chordKey !== shownChord) {
        shownChord = chordKey;
        chordSince = now;
        renderChord(frame.chord);
    } else if (frame.chord && now - chordSince >= historyStableMs) {
        remember(frame.chord, now);
    }
}

function frequencyX(frequency: number, width: number): number {
    return (Math.log(frequency / 50) / Math.log(6000 / 50)) * width;
}

function drawSpectrum(frame: Frame, now: number): void {
    if (!analyser || !audio) return;
    if (now - coloursReadAt > 1000) {
        colours = readColours();
        coloursReadAt = now;
    }
    const ratio = window.devicePixelRatio || 1;
    const width = Math.round(spectrum.clientWidth * ratio);
    const height = Math.round(spectrum.clientHeight * ratio);
    if (spectrum.width !== width || spectrum.height !== height) {
        spectrum.width = width;
        spectrum.height = height;
    }
    const context = spectrum.getContext("2d");
    if (!context || !width || !height) return;
    context.clearRect(0, 0, width, height);
    context.font = `${11 * ratio}px system-ui, sans-serif`;
    context.lineWidth = ratio;

    for (let midi = 24; midi <= 108; midi += 12) {
        const x = frequencyX(midiToHz(midi), width);
        context.strokeStyle = colours.line;
        context.beginPath();
        context.moveTo(x, 0);
        context.lineTo(x, height);
        context.stroke();
        context.fillStyle = colours.muted;
        context.fillText(noteName(midi), x + 3 * ratio, height - 5 * ratio);
    }

    for (const note of frame.notes) {
        context.strokeStyle = note.sounding ? colours.accent : colours.accentLine;
        for (let harmonic = 1; harmonic <= 8; harmonic += 1) {
            const x = frequencyX(note.frequency_hz * harmonic, width);
            if (x > width) break;
            context.globalAlpha = harmonic === 1 ? 1 : 0.35;
            context.lineWidth = (harmonic === 1 ? 2 : 1) * ratio;
            context.beginPath();
            context.moveTo(x, 0);
            context.lineTo(x, harmonic === 1 ? height : height * 0.25);
            context.stroke();
        }
    }
    context.globalAlpha = 1;
    context.lineWidth = 1.5 * ratio;

    analyser.getFloatFrequencyData(frequencyData);
    const binHz = audio.sampleRate / analyser.fftSize;
    const floorDb = -110;
    const ceilingDb = -10;
    const columns = new Float32Array(width).fill(-Infinity);
    for (let bin = Math.max(1, Math.floor(50 / binHz)); bin < frequencyData.length; bin += 1) {
        const x = Math.floor(frequencyX(bin * binHz, width));
        if (x >= width) break;
        if (x >= 0 && frequencyData[bin] > columns[x]) columns[x] = frequencyData[bin];
    }
    context.strokeStyle = colours.ink;
    context.beginPath();
    let drawing = false;
    for (let x = 0; x < width; x += 1) {
        if (columns[x] === -Infinity) continue;
        const y = height - ((columns[x] - floorDb) / (ceilingDb - floorDb)) * height;
        const clamped = Math.max(0, Math.min(height, y));
        if (drawing) context.lineTo(x, clamped);
        else context.moveTo(x, clamped);
        drawing = true;
    }
    context.stroke();
}

function tick(): void {
    if (mode === "off" || !analyser || !listener) {
        window.clearInterval(analysisTimer);
        analysisTimer = 0;
        return;
    }
    const now = performance.now();
    analyser.getFloatTimeDomainData(timeData);
    let frame: Frame;
    try {
        frame = listener.listen(timeData, now);
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
        stopSources();
        return;
    }
    lastFrame = frame;
    render(frame, now);
    updateStatus();
}

function paint(): void {
    frameRequest = 0;
    if (mode === "off") return;
    if (lastFrame) drawSpectrum(lastFrame, performance.now());
    frameRequest = requestAnimationFrame(paint);
}

function startLoop(): void {
    if (!analysisTimer) analysisTimer = window.setInterval(tick, analysisIntervalMs);
    if (!frameRequest) frameRequest = requestAnimationFrame(paint);
}

function ensureAudio(): { audio: AudioContext; analyser: AnalyserNode; listener: Listener } {
    if (!wasm) throw new Error("The analyzer has not loaded yet.");
    if (!audio || !analyser || !listener) {
        audio = new AudioContext({ latencyHint: "interactive" });
        analyser = audio.createAnalyser();
        analyser.smoothingTimeConstant = 0.6;
        analyser.minDecibels = -110;
        analyser.maxDecibels = -10;
        const silence = audio.createGain();
        silence.gain.value = 0;
        analyser.connect(silence).connect(audio.destination);
        listener = new wasm.ChordListener(audio.sampleRate);
        applySize();
        applySettings();
    }
    return { audio, analyser, listener };
}

function resumeAudio(context: AudioContext): void {
    if (context.state !== "running") context.resume().catch(() => undefined);
}

function applySize(): void {
    const size = Number(sizeSelect.value) || 8192;
    if (!analyser) return;
    analyser.fftSize = size;
    timeData = new Float32Array(size);
    frequencyData = new Float32Array(size / 2);
}

function applySettings(): void {
    const hold = Number(holdInput.value);
    const sensitivity = Number(sensitivityInput.value);
    const gate = Number(gateInput.value);
    holdValue.textContent = hold === 0 ? "off" : `${(hold / 1000).toFixed(1)} s`;
    sensitivityValue.textContent = `${Math.round(sensitivity * 100)}%`;
    gateValue.textContent = `${gate} dB`;
    meterGate.style.bottom = `${((gate + 90) / 90) * 100}%`;
    if (listener) {
        listener.set_hold_ms(hold);
        listener.set_sensitivity(sensitivity);
        listener.set_gate_db(gate);
    }
    const url = new URL(window.location.href);
    url.searchParams.set("hold", String(hold));
    url.searchParams.set("sensitivity", String(sensitivity));
    url.searchParams.set("gate", String(gate));
    url.searchParams.set("window", sizeSelect.value);
    window.history.replaceState({}, "", url.href);
}

function restoreSettings(): void {
    const params = new URLSearchParams(window.location.search);
    const restore = (input: HTMLInputElement | HTMLSelectElement, name: string) => {
        const value = params.get(name);
        if (value !== null && value !== "") input.value = value;
    };
    restore(holdInput, "hold");
    restore(sensitivityInput, "sensitivity");
    restore(gateInput, "gate");
    restore(sizeSelect, "window");
    if (!sizeSelect.value) sizeSelect.value = "8192";
}

function updateStatus(): void {
    let text = "Not listening";
    if (mode === "mic") {
        const label = stream?.getAudioTracks()[0]?.label;
        text = label ? `Listening to ${label}` : "Listening to the microphone";
    } else if (mode === "file") {
        text = player.paused ? `Paused: ${fileName}` : `Playing ${fileName}`;
    } else if (mode === "demo" && demo && audio) {
        const now = audio.currentTime;
        const current = demo.starts.filter((start) => start.at <= now).pop();
        text = current ? `Demo: ${current.label}` : "Demo starting";
    }
    if (statusNode.textContent !== text) statusNode.textContent = text;
    statusNode.classList.toggle("live", mode !== "off");
    micButton.textContent = mode === "mic" ? "Stop listening" : "Listen to microphone";
    demoButton.textContent = mode === "demo" ? "Stop demo" : "Play demo";
}

function stopSources(): void {
    if (micSource) {
        micSource.disconnect();
        micSource = null;
    }
    if (stream) {
        for (const track of stream.getTracks()) track.stop();
        stream = null;
    }
    if (demo) {
        window.clearInterval(demo.timer);
        for (const oscillator of demo.oscillators) {
            try {
                oscillator.stop();
            } catch {
                continue;
            }
        }
        demo.output.disconnect();
        demo = null;
    }
    if (fileSource) {
        player.pause();
        fileSource.disconnect();
    }
    mode = "off";
    updateStatus();
}

async function listDevices(): Promise<void> {
    if (!navigator.mediaDevices?.enumerateDevices) return;
    const devices = (await navigator.mediaDevices.enumerateDevices()).filter(
        (device) => device.kind === "audioinput" && device.deviceId,
    );
    const current = stream?.getAudioTracks()[0]?.getSettings().deviceId ?? deviceSelect.value;
    deviceSelect.replaceChildren(
        ...devices.map((device, index) => {
            const option = make("option", "", device.label || `Input ${index + 1}`);
            option.value = device.deviceId;
            return option;
        }),
    );
    if (current) deviceSelect.value = current;
    deviceField.hidden = devices.length < 2;
}

async function startMic(): Promise<void> {
    if (!navigator.mediaDevices?.getUserMedia) {
        fail("The microphone needs a secure page, served over https or from localhost.");
        return;
    }
    const { audio, analyser } = ensureAudio();
    stopSources();
    try {
        resumeAudio(audio);
        stream = await navigator.mediaDevices.getUserMedia({
            audio: {
                deviceId: deviceSelect.value ? { exact: deviceSelect.value } : undefined,
                echoCancellation: false,
                noiseSuppression: false,
                autoGainControl: false,
            },
        });
    } catch (err) {
        const name = err instanceof DOMException ? err.name : "";
        fail(
            name === "NotAllowedError"
                ? "Microphone access was not allowed. Allow it in the browser's site settings and try again."
                : name === "NotFoundError"
                  ? "No microphone was found."
                  : err instanceof Error
                    ? err.message
                    : String(err),
        );
        return;
    }
    clearError();
    micSource = audio.createMediaStreamSource(stream);
    micSource.connect(analyser);
    mode = "mic";
    updateStatus();
    startLoop();
    listDevices().catch(() => undefined);
}

function demoWave(context: AudioContext): PeriodicWave {
    const harmonics = 24;
    const real = new Float32Array(harmonics + 1);
    const imaginary = new Float32Array(harmonics + 1);
    for (let harmonic = 1; harmonic <= harmonics; harmonic += 1) {
        imaginary[harmonic] = Math.exp(-0.35 * (harmonic - 1)) * (harmonic % 2 ? 1 : 0.8);
    }
    return context.createPeriodicWave(real, imaginary);
}

function scheduleDemoStep(
    context: AudioContext,
    wave: PeriodicWave,
    output: GainNode,
    step: DemoStep,
    at: number,
    oscillators: OscillatorNode[],
): void {
    const end = at + demoStepSeconds - demoRestSeconds;
    const level = 0.6 / step.midi.length;
    step.midi.forEach((midi, index) => {
        const start = at + (step.arpeggio ? index * demoArpeggioSeconds : 0);
        const oscillator = context.createOscillator();
        oscillator.setPeriodicWave(wave);
        oscillator.frequency.value = midiToHz(midi);
        oscillator.detune.value = Math.random() * 8 - 4;
        const envelope = context.createGain();
        envelope.gain.setValueAtTime(0.0001, start);
        envelope.gain.exponentialRampToValueAtTime(level, start + 0.012);
        envelope.gain.exponentialRampToValueAtTime(level * 0.35, end);
        envelope.gain.exponentialRampToValueAtTime(0.0001, end + 0.15);
        oscillator.connect(envelope).connect(output);
        oscillator.start(start);
        oscillator.stop(end + 0.2);
        oscillators.push(oscillator);
        oscillator.addEventListener("ended", () => {
            envelope.disconnect();
            if (demo) demo.oscillators = demo.oscillators.filter((node) => node !== oscillator);
        });
    });
}

async function startDemo(): Promise<void> {
    const { audio, analyser } = ensureAudio();
    stopSources();
    resumeAudio(audio);
    clearError();
    const output = audio.createGain();
    output.connect(analyser);
    output.connect(audio.destination);
    const wave = demoWave(audio);
    let next = audio.currentTime + 0.15;
    let index = 0;
    const running = {
        output,
        timer: 0,
        oscillators: [] as OscillatorNode[],
        starts: [] as { at: number; label: string }[],
    };
    const scheduleAhead = () => {
        while (next < audio.currentTime + demoStepSeconds * 1.5) {
            const step = demoSteps[index % demoSteps.length];
            scheduleDemoStep(audio, wave, output, step, next, running.oscillators);
            running.starts.push({ at: next, label: step.label });
            next += demoStepSeconds;
            index += 1;
        }
        running.starts = running.starts.slice(-demoSteps.length);
    };
    scheduleAhead();
    running.timer = window.setInterval(scheduleAhead, 500);
    demo = running;
    mode = "demo";
    listener?.clear();
    updateStatus();
    startLoop();
}

async function startFile(file: File): Promise<void> {
    const { audio, analyser } = ensureAudio();
    stopSources();
    resumeAudio(audio);
    if (player.src) URL.revokeObjectURL(player.src);
    player.src = URL.createObjectURL(file);
    player.hidden = false;
    fileName = file.name;
    fileSource ??= audio.createMediaElementSource(player);
    fileSource.connect(analyser);
    fileSource.connect(audio.destination);
    mode = "file";
    listener?.clear();
    try {
        await player.play();
        clearError();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
    updateStatus();
    startLoop();
}

function clearAll(): void {
    listener?.clear();
    history = [];
    historyNode.replaceChildren();
    shownNotes = "";
    shownChord = "";
    renderNotes([]);
    renderTable([]);
    stage.classList.add("stale");
    symbolNode.textContent = "–";
    chordNameNode.textContent = "Play a chord, all at once or one note at a time.";
    facts.replaceChildren();
    pitches.replaceChildren();
    actions.replaceChildren();
}

micButton.addEventListener("click", () => {
    if (mode === "mic") stopSources();
    else startMic().catch((err) => fail(err instanceof Error ? err.message : String(err)));
});

demoButton.addEventListener("click", () => {
    if (mode === "demo") stopSources();
    else startDemo().catch((err) => fail(err instanceof Error ? err.message : String(err)));
});

fileInput.addEventListener("change", () => {
    const file = fileInput.files?.[0];
    if (file) startFile(file).catch((err) => fail(err instanceof Error ? err.message : String(err)));
    fileInput.value = "";
});

player.addEventListener("play", () => {
    if (mode !== "file" && fileSource && analyser && audio) {
        stopSources();
        fileSource.connect(analyser);
        fileSource.connect(audio.destination);
        mode = "file";
        startLoop();
    }
});

deviceSelect.addEventListener("change", () => {
    if (mode === "mic") startMic().catch((err) => fail(err instanceof Error ? err.message : String(err)));
});

clearButton.addEventListener("click", clearAll);

document.addEventListener("pointerdown", () => {
    if (audio && mode !== "off") resumeAudio(audio);
});

for (const input of [holdInput, sensitivityInput, gateInput]) {
    input.addEventListener("input", applySettings);
}

sizeSelect.addEventListener("change", () => {
    applySize();
    applySettings();
});

async function start(): Promise<void> {
    buildKeyboard();
    restoreSettings();
    applySettings();
    try {
        const module = (await import(
            new URL("../pkg/music21_rs_web.js", window.location.href).href
        )) as WasmModule;
        await module.default();
        wasm = module;
        micButton.disabled = false;
        demoButton.disabled = false;
        statusNode.textContent = "Not listening";
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

start();
