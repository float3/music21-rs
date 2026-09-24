import "../theme.js";
import { el, errorLine, fact } from "../dom.js";

// abcjs is loaded as a classic script and puts itself on `window.ABCJS`.
// Only the corners of its API this page touches are typed.
interface AbcPitch {
    pitch: number;
}

interface AbcElement {
    el_type: string;
    startChar?: number;
    endChar?: number;
    pitches?: AbcPitch[];
    abselem?: { elemset: SVGElement[] };
    chord?: { name: string; position?: string }[];
}

interface AbcStaff {
    voices: AbcElement[][];
    clef?: { type?: string };
    title?: string | string[];
}

interface AbcLine {
    staff?: AbcStaff[];
}

interface AudioNote {
    cmd: string;
    pitch: number;
    start: number;
    duration: number;
    volume?: number;
    startChar?: number | null;
    endChar?: number | null;
}

interface AbcTune {
    lines: AbcLine[];
    metaText: { title?: string; tempo?: { bpm?: number; duration?: number[] } };
    warnings?: string[];
    setUpAudio(): { tracks: AudioNote[][] };
    getKeySignature(): { root: string; acc: string; mode: string; accidentals?: { acc: string }[] };
    getMeterFraction(): { num: number; den: number };
    getPickupLength(): number;
    getBpm(): number;
    getBeatLength(): number;
    getSelectableArray?(): { absEl: { abcelem: AbcElement; elemset?: SVGElement[] }; svgEl: SVGElement }[];
}

interface AbcDrag {
    step: number;
}

interface NoteTimingEvent {
    elements?: SVGElement[][];
}

interface AbcjsGlobal {
    parseOnly(abc: string): AbcTune[];
    renderAbc(target: HTMLElement, abc: string, params: object): AbcTune[];
    strTranspose(abc: string, tunes: AbcTune[], steps: number): string;
    TimingCallbacks: new (
        tune: AbcTune,
        options: { eventCallback: (event: NoteTimingEvent | null) => void },
    ) => { start(): void; stop(): void };
    synth: {
        supportsAudio(): boolean;
        CreateSynth: new () => {
            init(options: object): Promise<unknown>;
            prime(): Promise<unknown>;
            start(): void;
            stop(): void;
            download(): string;
        };
    };
}

declare const ABCJS: AbcjsGlobal;

// What crosses into the crate: see `examples/web/src/score.rs`.
interface ScoreInput {
    title: string;
    tempo_bpm: number;
    key: { tonic: string; mode: string; sharps: number } | null;
    meter: [number, number] | null;
    pickup: number;
    let_ring: boolean;
    voices: {
        name: string;
        clef: string;
        notes: {
            start: number;
            duration: number;
            velocity: number;
            pitches: { midi: number; staff: number | null }[];
            char_start: number;
            char_end: number;
        }[];
    }[];
}

type Range = [number, number];

interface Slice {
    offset: number;
    duration: number;
    measure: number;
    beat: number;
    pitches: string[];
    pitch_classes: number[];
    common_name: string;
    pitched_common_name: string;
    chord_symbol: string | null;
    numeral: string | null;
    textbook_numeral: string | null;
    inversion: number | null;
    root: string | null;
    bass: string | null;
    consonant: boolean;
    changed: boolean;
    attacks: Range[];
    sounding: Range[];
    bass_attack: Range | null;
}

interface Issue {
    kind: string;
    severity: "warning" | "info";
    offset: number;
    measure: number;
    beat: number;
    voices: number[];
    detail: string;
    chars: Range[];
}

interface Analysis {
    title: string;
    tempo_bpm: number;
    meter: string;
    written_key: string | null;
    analysis_key: string | null;
    key_estimates: { key: string; score: number }[];
    tonal_certainty: number;
    measures: number;
    quarter_length: number;
    note_count: number;
    pitch_class_weights: number[];
    voices: {
        name: string;
        notes: number;
        lowest: string | null;
        highest: string | null;
        range: string | null;
        largest_leap: string | null;
    }[];
    slices: Slice[];
    /** `[text offset, label]` pairs to draw, by label kind. */
    labels: Record<string, [number, string][]>;
    issues: Issue[];
}

interface WasmModule {
    default(): Promise<unknown>;
    analyze_score(input: ScoreInput): Analysis;
    score_to_midi(input: ScoreInput): Uint8Array;
    score_to_musicxml(input: ScoreInput): string;
    midi_to_abc(bytes: Uint8Array): string;
    pitch_class_names(): string[];
    staff_midi(staff: number): number;
    abc_staff(token: string): number;
    abc_shift(token: string, steps: number): string;
    abc_accidental(staff: number, midi: number): string | undefined;
    written_octaves(staff: number, midi: number): number;
    abc_in_key(midi: number, key: ScoreInput["key"]): string;
    abc_key(root: string, acc: string, mode: string): { tonic: string; mode: string } | null;
    tab_strings(strings: string[], clef: string): Int32Array;
    tab_tuning(strings: string[], clef: string, positions: Int32Array): string[] | null;
    move_frets(strings: string[], clef: string, midis: Int32Array, frets: Int32Array, moved: number): [number, number][] | null;
    chord_part(
        input: ScoreInput,
        symbols: { start: number; name: string }[],
        strings: string[],
        clef: string,
    ): { start: number; abc: string }[];
    score_tuning_cents(
        input: ScoreInput,
        tuning: string,
        tonic: string,
    ): { pitch_class_cents: number[] | null; notes: number[][][] };
    playable_tuning_systems(): { id: string; name: string; description: string }[];
}

/** A note as abcjs's synth is about to sound it; `cents` detunes it. */
interface SynthNote {
    pitch: number;
    start: number;
    startChar?: number;
    endChar?: number;
    cents?: number;
}

const $ = <T extends HTMLElement>(selector: string): T => document.querySelector(selector) as T;

const textarea = $<HTMLTextAreaElement>("#abc");
const scoreNode = $<HTMLDivElement>("#score");
const statusNode = $<HTMLSpanElement>("#status");
const cursorNode = $<HTMLSpanElement>("#cursor");
const errorNode = $<HTMLParagraphElement>("#error");
const { fail, clearError } = errorLine(errorNode);
const momentNode = $<HTMLDivElement>("#moment");
const labelsSelect = $<HTMLSelectElement>("#labels");
const numeralsSelect = $<HTMLSelectElement>("#numerals");
const flagsButton = $<HTMLButtonElement>("#flags");
const ringButton = $<HTMLButtonElement>("#ring");
const chordsButton = $<HTMLButtonElement>("#chords");
const playButton = $<HTMLButtonElement>("#play");
const stopButton = $<HTMLButtonElement>("#stop");
const loopButton = $<HTMLButtonElement>("#loop");
const exportMenu = $<HTMLDetailsElement>("#export-menu");
const shareMenu = $<HTMLDetailsElement>("#share-menu");
const shareUrl = $<HTMLInputElement>("#share-url");
const shareNote = $<HTMLElement>("#share-note");
const viewSelect = $<HTMLSelectElement>("#view");
const stringsSelect = $<HTMLSelectElement>("#strings");
const temperamentSelect = $<HTMLSelectElement>("#temperament");
const rootSelect = $<HTMLSelectElement>("#temper-root");
const partsMenu = $<HTMLDetailsElement>("#parts-menu");
const partsNode = $<HTMLDivElement>("#parts");
const workspace = $<HTMLDivElement>(".workspace");
const wideButton = $<HTMLButtonElement>("#wide");
const zoomLevelButton = $<HTMLButtonElement>("#zoom-level");

const STORAGE_KEY = "music21-rs.score-editor";
const LABELS_KEY = "music21-rs.score-editor.labels";
const NUMERALS_KEY = "music21-rs.score-editor.numerals";
const VIEW_KEY = "music21-rs.score-editor.view";
const RING_KEY = "music21-rs.score-editor.ring";
const CHORDS_KEY = "music21-rs.score-editor.chords";
const ZOOM_KEY = "music21-rs.score-editor.zoom";
const WIDE_KEY = "music21-rs.score-editor.wide";
const ZOOMS = [0.4, 0.5, 0.67, 0.8, 1, 1.25, 1.5];
/** Tablature drawn under every staff, by the view's name, with its open
 * strings lowest first as abcjs writes them. Guitar and bass are tuned where
 * their music is written, an octave above where they sound; a staff whose
 * clef says so (`treble-8`) is tabbed against the sounding strings instead. */
interface StringTuning {
    id: string;
    name: string;
    notes: string[];
}
const TABLATURES: Record<string, { instrument: string; label: string; tunings: StringTuning[] }> = {
    guitar: {
        instrument: "guitar",
        label: "Guitar",
        tunings: [
            { id: "standard", name: "Standard (E A D G B E)", notes: ["E,", "A,", "D", "G", "B", "e"] },
            { id: "drop-d", name: "Drop D (D A D G B E)", notes: ["D,", "A,", "D", "G", "B", "e"] },
            { id: "half-down", name: "Half step down (E♭ A♭ D♭ G♭ B♭ E♭)", notes: ["_E,", "_A,", "_D", "_G", "_B", "_e"] },
            { id: "drop-c", name: "Drop C (C G C F A D)", notes: ["C,", "G,", "C", "F", "A", "d"] },
            { id: "dadgad", name: "DADGAD", notes: ["D,", "A,", "D", "G", "A", "d"] },
            { id: "open-g", name: "Open G (D G D G B D)", notes: ["D,", "G,", "D", "G", "B", "d"] },
            { id: "open-d", name: "Open D (D A D F♯ A D)", notes: ["D,", "A,", "D", "^F", "A", "d"] },
            { id: "open-e", name: "Open E (E B E G♯ B E)", notes: ["E,", "B,", "E", "^G", "B", "e"] },
            { id: "open-c", name: "Open C (C G C G C E)", notes: ["C,", "G,", "C", "G", "c", "e"] },
        ],
    },
    bass: {
        instrument: "guitar",
        label: "Bass",
        tunings: [
            { id: "standard", name: "Standard (E A D G)", notes: ["E,,", "A,,", "D,", "G,"] },
            { id: "drop-d", name: "Drop D (D A D G)", notes: ["D,,", "A,,", "D,", "G,"] },
            { id: "five-string", name: "Five-string (B E A D G)", notes: ["B,,,", "E,,", "A,,", "D,", "G,"] },
            { id: "half-down", name: "Half step down (E♭ A♭ D♭ G♭)", notes: ["_E,,", "_A,,", "_D,", "_G,"] },
        ],
    },
    mandolin: {
        instrument: "mandolin",
        label: "Mandolin",
        tunings: [{ id: "standard", name: "Standard (G D A E)", notes: ["G,", "D", "A", "e"] }],
    },
    fiddle: {
        instrument: "fiddle",
        label: "Fiddle",
        tunings: [
            { id: "standard", name: "Standard (G D A E)", notes: ["G,", "D", "A", "e"] },
            { id: "aeae", name: "Cross A (A E A E)", notes: ["A,", "E", "A", "e"] },
            { id: "adae", name: "Sawmill (A D A E)", notes: ["A,", "D", "A", "e"] },
            { id: "gdgd", name: "Cross G (G D G D)", notes: ["G,", "D", "G", "d"] },
        ],
    },
};
const STRINGS_KEY = "music21-rs.score-editor.strings";
const TUNING_KEY = "music21-rs.score-editor.tuning";
const ROOT_KEY = "music21-rs.score-editor.tuning-root";
/** The id `score_tuning_cents` gives chord-aware just intonation. */
const ADAPTIVE_TUNING = "RecursiveJustIntonation";
/** How many of abcjs's drag steps separate two tab strings. */
const DRAG_STEPS_PER_STRING = 3;

/** Example scores; `view` is the staff view one opens in. */
const EXAMPLES: { name: string; abc: string; view?: string }[] = [
    {
        name: "Four-part chorale",
        abc: `X:1
T:Chorale in C
M:4/4
L:1/4
Q:1/4=76
K:C
%%score (S A) (T B)
V:S clef=treble name="Upper"
V:A clef=treble
V:T clef=bass name="Lower"
V:B clef=bass
[V:S] e f d c | c d B c | c A e d | c4 |]
[V:A] G c B G | A A F E | G F G G | E4 |]
[V:T] C A, G, E, | E, D, D, G, | C C C B, | C4 |]
[V:B] C, F,, G,, C, | A,, F,, G,, C, | E, F, G,, G,, | C,4 |]
`,
    },
    {
        name: "Waltz melody",
        abc: `X:2
T:Waltz sketch
M:3/4
L:1/8
Q:1/4=132
K:G
D2 | "G"G2 B2 d2 | "C"e4 d2 | "D7"c2 A2 F2 | "G"G4 D2 |
"Em"E2 G2 B2 | "Am"c3 B A2 | "D7"F2 A2 c2 | "G"B4 |]
`,
    },
    {
        name: "Piano study in A minor",
        abc: `X:3
T:Minor study
M:4/4
L:1/8
Q:1/4=88
K:Am
%%score {RH LH}
V:RH clef=treble name="Piano"
V:LH clef=bass
[V:RH] (3cBA (3EAc e2 dc | B2 ^G2 A4- | A2 (3ABc d2 e2 | f2 ^G2 A4 |]
[V:LH] [A,,E,]4 [A,,E,]4 | [E,,E,]4 [A,,E,]4 | [D,F,]4 [D,A,]4 | [E,^G,]4 [A,,A,]4 |]
`,
    },
    {
        name: "Two-finger bossa nova",
        view: "guitar",
        abc: `X:4
T:Two-finger bossa nova
M:4/4
L:1/8
Q:1/4=120
K:Bm
%%MIDI program 24
V:1 clef=treble-8 name="Guitar"
F,2 [DAB]2 F, [DAB]2 F, | =F,2 [D^GB] z [DGB]4 | E,2 [DGB]2 E, [DGB]2 z | A,2 [EGc]2 z [EGc] E,=F, |
F,2 [DAB]2 F, [DAB]2 F, | =F,2 [D^GB] z [DGB]4 | E,2 [DGB]2 E, [DGB]2 z | A,2 [EGc] z [EGc]4 |]
`,
    },
    {
        name: "Un bossa +",
        view: "guitar",
        abc: `X:5
T:Un bossa +
M:4/4
L:1/8
Q:1/4=130
K:C
%%MIDI program 24
P:A
D E F G | "Dm7" A4 A2 z2 | "G" B4 c B A G | "Cmaj7" e2 d c B2 z2 | z4 z D E F |
w: hoy que no a-guan-to más quie-ro em-pe-zar de ce-ro y con un
"Dm7" A4 F4 | "G" G4 B c d d | "Cmaj7" e4 c2 z2 | z4 D E F G |
w: bos-sa más quie-ro que bai-le-mos por-que en la
P:B
"Fmaj7" A4 F2 E F | "Fm6" c3 _A G =A B c | "Cmaj7" e3 d c z2 B | "A7" ^c4 A2 z E |
w: no-che que te fuis-te no fue el fi-nal que vos qui-si-ste bai-
"Dm7" A4 F2 z E | "G" d4 B4 | "Cmaj7" c8 | z6 G A |
w: le-mos un bos-sa más por-que
P:C
"Dm7" A4 F2 z D | "G" B4 z3 d | "Cmaj7" e8 | z8 |
w: cuan-do es-toy con vos
"Dm7" A4 A4 | "G" B4 G2 z A | "Gm7" _B8 | "C9" z4 D E F G |
w: to-do nues-tro a-mor y no ha-ce
"Fmaj7" A4 F G A G | "Fm6" _A2 F2 G =A B c | "Cmaj7" e4 d c z B | "A7" ^c4 A2 z E |
w: fal-ta que me ex-pli-ques bas-ta que es-tés pa-ra de-cir-te bai-
"Dm7" A4 F2 z E | "G" d4 B4 | "Gm7" _B8 | "C9" z7 E |
w: le-mos un bos-sa más bai-
"Fmaj7" A4 F G A z | "Fm6" _A4 z3 F | "Cmaj7" G4 E F G z | "A7" A4 z3 E |
w: le-mos o-tro más bai-le-mos o-tro más bai-
"Dm7" A4 F2 z E | "G" d4 B4 | "Cmaj7" c8 | z8 |]
w: le-mos un bos-sa más
`,
    },
    {
        name: "Blank",
        abc: `X:1
T:Untitled
M:4/4
L:1/4
Q:1/4=100
K:C
C D E F | G2 G2 |]
`,
    },
];

let wasm: WasmModule | null = null;
let analysis: Analysis | null = null;
let scoreInput: ScoreInput | null = null;
let rendered: Rendered | null = null;
let refreshTimer = 0;
/** The note last picked in the score, and whether it was its tab number. */
let selected: { anchor: number; tab: boolean } | null = null;
let refocusScore = false;
/** Why staves of the drawn score got no tablature. */
let tabSkipped: string[] = [];
let zoom = 1;
/** Voice ids left out of the drawn score. */
const hiddenParts = new Set<string>();
let player: { synth: InstanceType<AbcjsGlobal["synth"]["CreateSynth"]>; timer: { stop(): void } } | null = null;

// ---------------------------------------------------------------- helpers

function formatBeat(beat: number): string {
    return Number.isInteger(beat) ? String(beat) : String(Math.round(beat * 100) / 100);
}

function storageGet(key: string): string | null {
    try {
        return window.localStorage.getItem(key);
    } catch {
        return null;
    }
}

function storageSet(key: string, value: string): void {
    try {
        window.localStorage.setItem(key, value);
    } catch {
        // Nothing is lost for this visit when storage is unavailable.
    }
}

function download(name: string, data: BlobPart, type: string): void {
    const url = URL.createObjectURL(new Blob([data], { type }));
    const link = el("a");
    link.href = url;
    link.download = name;
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

function fileStem(): string {
    const title = analysis?.title || "score";
    return title.replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "").toLowerCase() || "score";
}

/** Replaces part of the source through the browser's editing commands, so
 * the change lands on the textarea's own undo stack. */
function replaceRange(start: number, end: number, text: string): void {
    textarea.focus({ preventScroll: true });
    textarea.setSelectionRange(start, end);
    let done = false;
    try {
        done = document.execCommand("insertText", false, text);
    } catch {
        done = false;
    }
    if (!done) {
        textarea.setRangeText(text, start, end, "end");
        textarea.dispatchEvent(new Event("input"));
    }
}

// ------------------------------------------------------------- ABC text

function isMusicLine(line: string): boolean {
    return !/^\s*([A-Za-z+]:|%)/.test(line);
}

/** Calls `visit` for each stretch of a music line that is ABC notation
 * rather than a quoted string, an inline field, a comment or a decoration.
 * Decorations and grace groups are reported separately with `blank`. */
function scanMusic(
    abc: string,
    visit: (index: number) => void,
    blank?: (start: number, end: number) => void,
): void {
    let lineStart = 0;
    while (lineStart <= abc.length) {
        const newline = abc.indexOf("\n", lineStart);
        const lineEnd = newline === -1 ? abc.length : newline;
        const line = abc.slice(lineStart, lineEnd);
        if (isMusicLine(line)) {
            let i = lineStart;
            while (i < lineEnd) {
                const ch = abc[i];
                if (ch === "%") break;
                if (ch === '"') {
                    const close = abc.indexOf('"', i + 1);
                    i = close === -1 || close > lineEnd ? lineEnd : close + 1;
                    continue;
                }
                if (ch === "[" && /^\[[A-Za-z]:/.test(abc.slice(i, i + 3))) {
                    const close = abc.indexOf("]", i);
                    i = close === -1 || close > lineEnd ? lineEnd : close + 1;
                    continue;
                }
                if (ch === "!" || ch === "+" || ch === "{") {
                    const closer = ch === "{" ? "}" : ch;
                    const close = abc.indexOf(closer, i + 1);
                    if (close !== -1 && close < lineEnd) {
                        blank?.(i, close + 1);
                        i = close + 1;
                        continue;
                    }
                }
                if ("~HLMOPSTuv".includes(ch) && /^[~HLMOPSTuv.]*[\^_=[A-Ga-g]/.test(abc.slice(i + 1, i + 8))) {
                    if (i === lineStart || !/[A-Za-z]/.test(abc[i - 1])) {
                        blank?.(i, i + 1);
                        i += 1;
                        continue;
                    }
                }
                visit(i);
                i += 1;
            }
        }
        if (newline === -1) break;
        lineStart = newline + 1;
    }
}

/** The source with ornaments and grace notes replaced by spaces. abcjs plays
 * a trill as a run of notes with no place in the text and drops the note
 * written, which would leave the analysis without it; blanking them keeps
 * every other character where it was. */
function withoutOrnaments(abc: string): string {
    const chars = abc.split("");
    scanMusic(
        abc,
        () => undefined,
        (start, end) => {
            for (let i = start; i < end; i++) if (chars[i] !== "\n") chars[i] = " ";
        },
    );
    return chars.join("");
}

/** The offset of the first pitch, rest or chord bracket inside an abcjs
 * element: the place a label goes and the one offset that is the same in
 * the source, the source without ornaments and the labelled copy. */
function anchorOf(abc: string, start: number, end: number): number {
    let i = start;
    while (i < end) {
        const ch = abc[i];
        if (ch === '"' || ch === "!" || ch === "+" || ch === "{") {
            const closer = ch === "{" ? "}" : ch;
            const close = abc.indexOf(closer, i + 1);
            if (close === -1 || close >= end) return start;
            i = close + 1;
            continue;
        }
        if (ch === "(") {
            i += 1;
            while (i < end && /[0-9:]/.test(abc[i])) i += 1;
            continue;
        }
        if (/[\^_=[A-Ga-gzxZ]/.test(ch)) return i;
        i += 1;
    }
    return start;
}

/** Where a note token ends: its pitches, octave marks, length and tie. */
function tokenEnd(abc: string, anchor: number): number {
    const match = /^(\[[^\]]*\]|[\^_=]*[A-Ga-gzxZ][,']*)[0-9/]*-?/.exec(abc.slice(anchor));
    return anchor + (match ? match[0].length : 1);
}

// --------------------------------------------------------- abcjs → crate

function keyOf(tune: AbcTune, module: WasmModule): ScoreInput["key"] {
    const signature = tune.getKeySignature();
    const key = signature ? module.abc_key(signature.root, signature.acc, signature.mode ?? "") : null;
    if (!key) return null;
    const sharps = (signature.accidentals ?? []).reduce(
        (sum, accidental) => sum + (accidental.acc === "sharp" ? 1 : accidental.acc === "flat" ? -1 : 0),
        0,
    );
    return { ...key, sharps };
}

function quarterTempo(tune: AbcTune): number {
    const tempo = tune.metaText.tempo;
    if (tempo?.bpm && tempo.duration?.length) {
        return tempo.bpm * tempo.duration.reduce((sum, part) => sum + part, 0) * 4;
    }
    const bpm = tune.getBpm();
    const beat = tune.getBeatLength();
    return bpm > 0 && beat > 0 ? bpm * beat * 4 : 120;
}

/** The voice ids the source declares, in the order abcjs lays them out:
 * the order of a `%%score` line when there is one, otherwise the order the
 * `V:` fields first appear in. */
function voiceIds(abc: string): string[] {
    const declared: string[] = [];
    for (const match of abc.matchAll(/(?:^|\n)\s*V:\s*([^\s\]]+)|\[V:\s*([^\s\]]+)/g)) {
        const id = match[1] ?? match[2];
        if (id && !declared.includes(id)) declared.push(id);
    }
    const layout = /(?:^|\n)%%(?:score|staves)\s+([^\n]*)/.exec(abc);
    if (!layout) return declared;
    const ordered = (layout[1].match(/[^\s(){}[\]|]+/g) ?? []).filter((id) => declared.includes(id));
    return [...ordered, ...declared.filter((id) => !ordered.includes(id))];
}

/** Reads what abcjs made of the source into the crate's score shape. */
function extract(abc: string): ScoreInput | null {
    if (!wasm) return null;
    const module = wasm;
    const clean = withoutOrnaments(abc);
    const tune = ABCJS.parseOnly(clean)[0];
    if (!tune) return null;

    const elements = new Map<number, AbcElement>();
    const voices: { name: string; clef: string }[] = [];
    for (const line of tune.lines) {
        if (!line.staff) continue;
        let index = 0;
        for (const staff of line.staff) {
            staff.voices.forEach((voice, v) => {
                if (!voices[index]) {
                    const title = Array.isArray(staff.title) ? staff.title[v] : v === 0 ? staff.title : "";
                    voices[index] = { name: title ?? "", clef: staff.clef?.type ?? "" };
                }
                for (const element of voice) {
                    if (element.el_type === "note" && element.pitches && element.startChar != null) {
                        elements.set(element.startChar, element);
                    }
                }
                index += 1;
            });
        }
    }

    const audio = tune.setUpAudio();
    const ids = voiceIds(abc);
    const input: ScoreInput = {
        title: tune.metaText.title ?? "",
        tempo_bpm: quarterTempo(tune),
        key: keyOf(tune, module),
        meter: null,
        pickup: tune.getPickupLength() * 4,
        let_ring: ringButton.classList.contains("on"),
        voices: [],
    };
    const meter = tune.getMeterFraction();
    if (meter && Number.isFinite(meter.num) && Number.isFinite(meter.den) && meter.num > 0) {
        input.meter = [meter.num, meter.den];
    }

    let voiceIndex = 0;
    for (const track of audio.tracks) {
        const written = track.filter((event) => event.cmd === "note" && event.startChar != null);
        // A track of chord-symbol accompaniment has no place in the text.
        if (written.length === 0) continue;
        const meta = voices[voiceIndex] ?? { name: "", clef: "" };
        voiceIndex += 1;

        const groups = new Map<string, AudioNote[]>();
        for (const event of written) {
            const id = `${event.startChar}:${event.start}`;
            const group = groups.get(id);
            if (group) group.push(event);
            else groups.set(id, [event]);
        }
        const notes: ScoreInput["voices"][number]["notes"] = [];
        for (const group of groups.values()) {
            const first = group[0];
            const element = elements.get(first.startChar as number);
            // Written and sounding pitches pair up in order, which holds
            // on a staff that sounds an octave from where it is written;
            // nearest-first pairing is only for a chord abcjs sounded with
            // fewer or more notes than it wrote.
            const written = (element?.pitches ?? []).map((pitch) => pitch.pitch).sort((a, b) => a - b);
            const sounding = [...group].sort((a, b) => a.pitch - b.pitch);
            const unused = [...written];
            const pitches = sounding.map((event, index) => {
                if (written.length === sounding.length) return { midi: event.pitch, staff: written[index] };
                let best = -1;
                unused.forEach((staff, i) => {
                    const distance = Math.abs(module.staff_midi(staff) - event.pitch);
                    if (best === -1 || distance < Math.abs(module.staff_midi(unused[best]) - event.pitch)) best = i;
                });
                return { midi: event.pitch, staff: best === -1 ? null : unused.splice(best, 1)[0] };
            });
            const start = first.startChar as number;
            const end = element?.endChar ?? first.endChar ?? start + 1;
            notes.push({
                start: first.start * 4,
                duration: Math.max(...group.map((event) => event.duration)) * 4,
                velocity: Math.max(1, Math.min(127, first.volume ?? 90)),
                pitches,
                char_start: anchorOf(abc, start, end),
                char_end: end,
            });
        }
        const id = ids[voiceIndex - 1];
        const name = meta.name || (id && !/^\d+$/.test(id) ? id : `Voice ${voiceIndex}`);
        input.voices.push({ name, clef: meta.clef, notes });
    }
    return input;
}

// ------------------------------------------------------------- rendering

interface Rendered {
    tune: AbcTune;
    text: string;
    insertions: { at: number; length: number }[];
    /** Rendered elements by the anchor of their first pitch in the source. */
    byAnchor: Map<number, AbcElement[]>;
    /** Whether an offset of the drawn text is in the generated chord part,
     * which is not in the source. */
    isGenerated: (position: number) => boolean;
    /** Whether the chord symbols are drawn as a part of their own, which then
     * plays them in place of abcjs's accompaniment. */
    chordPart: boolean;
}

/** A slice's Roman numeral in the style chosen: as a harmony textbook writes
 * it, or exactly as music21 does. */
function numeralOf(slice: Slice): string | null {
    return numeralsSelect.value === "music21" ? slice.numeral : slice.textbook_numeral;
}

function toSource(insertions: Rendered["insertions"], position: number): number {
    let shift = 0;
    for (const insertion of insertions) {
        const at = insertion.at + shift;
        if (at >= position) break;
        if (position < at + insertion.length) return insertion.at;
        shift += insertion.length;
    }
    return position - shift;
}

/** The label kind the crate places labels for, from the labels menu. */
function labelKind(): string {
    return labelsSelect.value === "numeral" ? numeralsSelect.value : labelsSelect.value;
}

function render(source: string): void {
    const labels = analysis?.labels[labelKind()] ?? [];
    // What is drawn is the source with text inserted and nothing taken out,
    // so every rendered offset maps back: a hidden part's lines are
    // commented out with a `%` at their start, and labels go before notes.
    const additions: { at: number; text: string; order: number; generated?: boolean }[] = hiddenLineStarts(
        source,
    ).map((at) => ({ at, text: "%", order: 0 }));
    const chords = chordPart(source);
    for (const addition of chords ?? []) additions.push({ ...addition, order: 1, generated: true });
    for (const [at, label] of labels) additions.push({ at, text: `"_${label.replace(/"/g, "'")}"`, order: 2 });
    additions.sort((a, b) => a.at - b.at || a.order - b.order);
    const insertions: Rendered["insertions"] = [];
    const generated: [number, number][] = [];
    let text = "";
    let from = 0;
    for (const addition of additions) {
        text += source.slice(from, addition.at);
        if (addition.generated) generated.push([text.length, text.length + addition.text.length]);
        text += addition.text;
        insertions.push({ at: addition.at, length: addition.text.length });
        from = addition.at;
    }
    text += source.slice(from);
    const isGenerated = (position: number) => generated.some(([start, end]) => position >= start && position < end);

    const tablature = TABLATURES[viewSelect.value];
    tabSkipped = [];
    const accent = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#0f766e";
    const width = Math.max(320, scoreNode.clientWidth - 28);
    const scrollTop = scoreNode.scrollTop;
    const tune = ABCJS.renderAbc(scoreNode, text, {
        ...(tablature && wasm ? { tablature: tablatureFor(text, tablature, stringTuning()) } : {}),
        add_classes: true,
        responsive: "resize",
        staffwidth: Math.round(width / zoom),
        // abcjs draws no tablature on a tune it has rewrapped, so a tab view
        // keeps the lines as written and zoom only scales it.
        ...(tablature ? {} : { wrap: { minSpacing: 1.8, maxSpacing: 2.7, preferredMeasuresPerLine: 4 } }),
        foregroundColor: "currentColor",
        paddingbottom: 12,
        dragging: true,
        selectTypes: ["note", "tabNumber"],
        selectionColor: accent,
        dragColor: accent,
        clickListener: (element: AbcElement, _tune: number, _classes: string, _analysis: unknown, drag?: AbcDrag) =>
            onScoreClick(element, drag),
    })[0];

    const byAnchor = new Map<number, AbcElement[]>();
    for (const line of tune?.lines ?? []) {
        for (const staff of line.staff ?? []) {
            for (const voice of staff.voices) {
                for (const element of voice) {
                    if (element.el_type !== "note" || element.startChar == null || element.endChar == null) continue;
                    if (isGenerated(element.startChar)) continue;
                    const start = toSource(insertions, element.startChar);
                    const end = toSource(insertions, element.endChar);
                    const anchor = anchorOf(source, start, end);
                    const list = byAnchor.get(anchor);
                    if (list) list.push(element);
                    else byAnchor.set(anchor, [element]);
                }
            }
        }
    }
    rendered = tune ? { tune, text, insertions, byAnchor, isGenerated, chordPart: chords !== null } : null;
    fitScore();
    scoreNode.scrollTop = scrollTop;
    markIssues();
    if (refocusScore) {
        refocusScore = false;
        focusSelected();
    }

    const warnings = tune?.warnings?.length ?? 0;
    statusNode.replaceChildren();
    if (warnings > 0) {
        const bad = el("span", "bad", `${warnings} parse warning${warnings === 1 ? "" : "s"}`);
        bad.title = (tune?.warnings ?? []).map((warning) => warning.replace(/<[^>]+>/g, "")).join("\n");
        statusNode.append(bad, ` · `);
    }
    statusNode.append(
        analysis
            ? `${analysis.note_count} notes · ${analysis.measures} bars · ${analysis.voices.length} voice${analysis.voices.length === 1 ? "" : "s"}`
            : "No notes",
    );
    if (tabSkipped.length > 0) {
        const note = el("span", "bad", ` · no tab on ${tabSkipped.length} staff${tabSkipped.length === 1 ? "" : "s"}`);
        note.title = tabSkipped.map((reason) => `No tablature: ${reason}.`).join("\n");
        statusNode.append(note);
    }
}

/** Grows the drawn area to everything abcjs actually drew. abcjs measures a
 * responsive score before it adds the tablature, so the last tab staff falls
 * outside the viewBox and is cut off at any zoom. */
function fitScore(): void {
    const svg = scoreNode.querySelector("svg");
    if (!svg) return;
    const box = svg.getBBox();
    const view = svg.viewBox.baseVal;
    const height = Math.ceil(box.y + box.height + 8);
    if (!view.width || height <= view.height) return;
    svg.setAttribute("viewBox", `${view.x} ${view.y} ${view.width} ${height}`);
    const container = svg.parentElement;
    if (container?.classList.contains("abcjs-container")) {
        container.style.paddingBottom = `${(height / view.width) * 100}%`;
    }
}

function svgOf(elements: AbcElement[] | undefined): SVGElement[] {
    return (elements ?? []).flatMap((element) => element.abselem?.elemset ?? []);
}

function setClass(className: string, anchors: number[]): void {
    scoreNode.querySelectorAll(`.${className}`).forEach((node) => node.classList.remove(className));
    for (const anchor of anchors) {
        for (const node of svgOf(rendered?.byAnchor.get(anchor))) node.classList.add(className);
    }
}

function markIssues(): void {
    const on = flagsButton.classList.contains("on");
    const anchors = on
        ? (analysis?.issues ?? []).filter((issue) => issue.severity === "warning").flatMap((issue) => issue.chars.map((range) => range[0]))
        : [];
    setClass("m21-flag", anchors);
}

// ------------------------------------------------------------- selection

function revealInSource(start: number, end: number, focus = true): void {
    if (focus) textarea.focus({ preventScroll: true });
    textarea.setSelectionRange(start, end);
    const line = textarea.value.slice(0, start).split("\n").length - 1;
    const lineHeight = parseFloat(getComputedStyle(textarea).lineHeight) || 20;
    const top = line * lineHeight;
    if (top < textarea.scrollTop || top > textarea.scrollTop + textarea.clientHeight - lineHeight * 2) {
        textarea.scrollTop = Math.max(0, top - textarea.clientHeight / 3);
    }
}

/** The source anchor of an element abcjs rendered from the labelled copy. */
function anchorOfElement(element: AbcElement): number | null {
    if (!rendered || element.startChar == null || element.endChar == null) return null;
    if (rendered.isGenerated(element.startChar)) return null;
    const start = toSource(rendered.insertions, element.startChar);
    const end = toSource(rendered.insertions, element.endChar);
    return anchorOf(textarea.value, start, end);
}

function selectElement(element: AbcElement): number | null {
    const anchor = anchorOfElement(element);
    if (anchor === null) return null;
    selected = { anchor, tab: element.el_type === "tabNumber" };
    revealInSource(anchor, tokenEnd(textarea.value, anchor), false);
    syncSelection();
    return anchor;
}

/** A click or a drag in the score. A drag has moved the note `step` staff
 * positions down, or a tab number that far down across the strings. */
function onScoreClick(element: AbcElement, drag?: AbcDrag): void {
    const anchor = selectElement(element);
    const step = drag?.step ?? 0;
    if (anchor === null || step === 0) return;
    refocusScore = true;
    if (element.el_type === "tabNumber") {
        const strings = -Math.round(step / DRAG_STEPS_PER_STRING);
        if (strings === 0 || !moveAcrossStrings(anchor, strings)) render(textarea.value);
    } else {
        shiftNote(anchor, -step);
    }
}

/** Gives keyboard focus back to the selected note after the score redraws. */
function focusSelected(): void {
    if (!rendered || !selected) return;
    const want = selected;
    // A tab number turned into a rest has no tab number left, so the rest
    // itself takes the focus.
    const candidates = (rendered.tune.getSelectableArray?.() ?? []).filter(
        (item) => anchorOfElement(item.absEl.abcelem) === want.anchor,
    );
    const match =
        candidates.find((item) => (item.absEl.abcelem.el_type === "tabNumber") === want.tab) ?? candidates[0];
    if (match) (match.svgEl as SVGElement & { focus(options?: FocusOptions): void }).focus({ preventScroll: true });
}

function focusAnchors(anchors: number[]): void {
    setClass("m21-focus", anchors);
    for (const item of rendered?.tune.getSelectableArray?.() ?? []) {
        if (item.absEl.abcelem.el_type !== "tabNumber") continue;
        const anchor = anchorOfElement(item.absEl.abcelem);
        if (anchor !== null && anchors.includes(anchor)) item.svgEl.classList.add("m21-focus");
    }
}

function sliceAt(anchor: number): Slice | null {
    const slices = analysis?.slices ?? [];
    return (
        slices.find((slice) => slice.attacks.some((range) => range[0] === anchor)) ??
        slices.find((slice) => slice.sounding.some((range) => range[0] === anchor)) ??
        null
    );
}

function showMoment(anchor: number): void {
    const slice = sliceAt(anchor);
    if (!slice) {
        momentNode.replaceChildren(el("p", "", "No harmony sounds at this note."));
        return;
    }
    const facts = el("div", "facts");
    facts.append(
        fact("Bar · beat", `${slice.measure} · ${formatBeat(slice.beat)}`),
        fact("Pitches", slice.pitches.join(" ")),
        fact("Chord", slice.pitched_common_name),
    );
    if (slice.chord_symbol) facts.append(fact("Symbol", slice.chord_symbol));
    const numeral = numeralOf(slice);
    if (numeral) facts.append(fact(`Numeral in ${analysis?.analysis_key ?? "key"}`, numeral));
    if (slice.root) facts.append(fact("Root · bass", `${slice.root} · ${slice.bass ?? "–"}`));
    const link = el("a", "", "Open in Chord Inspector");
    const params = new URLSearchParams({ chord: slice.pitches.join(" ") });
    const key = analysis?.analysis_key?.split(" ");
    if (key) params.set("key", key[1] === "minor" ? key[0].toLowerCase() : key[0]);
    link.href = `../chord/?${params}`;
    const linkFact = el("div", "fact");
    linkFact.append(el("span", "", "More"), link);
    facts.append(linkFact);
    momentNode.replaceChildren(facts);
}

/** Every note whose anchor lies inside the textarea's selection. */
function anchorsInSelection(): number[] {
    if (!rendered) return [];
    const { selectionStart, selectionEnd } = textarea;
    const anchors = [...rendered.byAnchor.keys()];
    if (selectionStart === selectionEnd) {
        const before = anchors.filter((anchor) => anchor <= selectionStart && tokenEnd(textarea.value, anchor) >= selectionStart);
        return before.slice(-1);
    }
    return anchors.filter((anchor) => anchor >= selectionStart && anchor < selectionEnd);
}

function syncSelection(): void {
    const value = textarea.value;
    const before = value.slice(0, textarea.selectionStart).split("\n");
    cursorNode.textContent = `Ln ${before.length}, Col ${before[before.length - 1].length + 1}`;
    const anchors = anchorsInSelection();
    focusAnchors(anchors);
    if (anchors.length > 0) showMoment(anchors[0]);
    else momentNode.replaceChildren(el("p", "", "Click a note to see the harmony sounding there."));
}

// --------------------------------------------------------------- editing

type NoteInput = ScoreInput["voices"][number]["notes"][number];
type Expected = Map<number, { midi: number; staff: number | null }[]>;

const PITCH_TOKEN = /(\^\^|\^|__|_|=)?([A-Ga-g])([,']*)/g;

/** The loaded crate: every pitch an edit reads or writes is its to decide. */
function crate(): WasmModule {
    if (!wasm) throw new Error("music21-rs has not loaded yet.");
    return wasm;
}

/** The first time each note is heard, by its anchor: repeats play it again. */
function notesByAnchor(input: ScoreInput | null): Map<number, NoteInput & { clef: string }> {
    const notes = new Map<number, NoteInput & { clef: string }>();
    for (const voice of input?.voices ?? []) {
        for (const note of voice.notes) if (!notes.has(note.char_start)) notes.set(note.char_start, { ...note, clef: voice.clef });
    }
    return notes;
}

/** The pitches written in the note at `anchor`: where each starts (at its
 * accidental), where its letter is, and its staff position. */
function pitchTokens(text: string, anchor: number): { start: number; letter: number; end: number; staff: number }[] {
    const close = text[anchor] === "[" ? text.indexOf("]", anchor) : -1;
    const regionEnd = close === -1 ? tokenEnd(text, anchor) : close;
    const tokens = [];
    PITCH_TOKEN.lastIndex = 0;
    for (const match of text.slice(anchor, regionEnd).matchAll(PITCH_TOKEN)) {
        const start = anchor + (match.index ?? 0);
        if (close === -1 && start !== anchor) break;
        const letter = start + (match[1]?.length ?? 0);
        tokens.push({ start, letter, end: start + match[0].length, staff: crate().abc_staff(match[2] + match[3]) });
        if (close === -1) break;
    }
    return tokens;
}

/** Rewrites accidentals until every note in `expected` sounds what it is
 * expected to. An edit can change a note it never touched: an accidental
 * carries to the end of the bar, so taking one away or adding one moves
 * the notes after it. Each such note gets an explicit accidental back. */
function keepSounding(text: string, expected: Expected): string {
    let wanted = expected;
    for (let pass = 0; pass < 4; pass++) {
        const actual = notesByAnchor(extract(text));
        const edits: { start: number; end: number; text: string }[] = [];
        for (const [anchor, want] of wanted) {
            const note = actual.get(anchor);
            if (!note) continue;
            const have = note.pitches.map((pitch) => pitch.midi).sort((a, b) => a - b);
            const need = want.map((pitch) => pitch.midi).sort((a, b) => a - b);
            if (have.length === need.length && have.every((midi, i) => midi === need[i])) continue;
            const used = new Set<number>();
            for (const token of pitchTokens(text, anchor)) {
                let pick = want.findIndex((pitch, i) => !used.has(i) && pitch.staff === token.staff);
                if (pick === -1) {
                    pick = want.findIndex(
                        (pitch, i) =>
                            !used.has(i) && pitch.staff === null && crate().abc_accidental(token.staff, pitch.midi) !== undefined,
                    );
                }
                if (pick === -1) continue;
                used.add(pick);
                const accidental = crate().abc_accidental(token.staff, want[pick].midi);
                if (accidental !== undefined && text.slice(token.start, token.letter) !== accidental) {
                    edits.push({ start: token.start, end: token.letter, text: accidental });
                }
            }
        }
        if (edits.length === 0) return text;
        edits.sort((a, b) => b.start - a.start);
        for (const change of edits) text = text.slice(0, change.start) + change.text + text.slice(change.end);
        const shifted: Expected = new Map();
        for (const [anchor, want] of wanted) {
            const delta = edits
                .filter((change) => change.start < anchor)
                .reduce((sum, change) => sum + change.text.length - (change.end - change.start), 0);
            shifted.set(anchor + delta, want);
        }
        wanted = shifted;
    }
    return text;
}

/** Replaces part of the source as one undoable edit, keeping every note the
 * edit does not mean to change sounding as it did, and redraws. `targets`
 * are notes of the new text that must sound given MIDI numbers. */
function applyEdit(start: number, end: number, replacement: string, targets = new Map<number, number[]>()): void {
    const before = textarea.value;
    const delta = replacement.length - (end - start);
    const expected: Expected = new Map();
    for (const [anchor, note] of notesByAnchor(scoreInput)) {
        if (anchor >= start && anchor < end) continue;
        expected.set(anchor >= end ? anchor + delta : anchor, note.pitches);
    }
    for (const [anchor, midis] of targets) expected.set(anchor, midis.map((midi) => ({ midi, staff: null })));
    const after = keepSounding(before.slice(0, start) + replacement + before.slice(end), expected);
    if (after === before) {
        render(before);
        return;
    }
    let prefix = 0;
    while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) prefix++;
    let suffix = 0;
    while (
        suffix < before.length - prefix &&
        suffix < after.length - prefix &&
        before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
    ) {
        suffix++;
    }
    const focusScore = refocusScore;
    replaceRange(prefix, before.length - suffix, after.slice(prefix, after.length - suffix));
    window.clearTimeout(refreshTimer);
    refocusScore = focusScore;
    refresh();
}

/** The written pitches between `start` and `end`, moved `steps` staff
 * positions. A step drops the accidental so the note follows the key; an
 * octave keeps it. */
function shiftedPitches(value: string, start: number, end: number, steps: number): string {
    const inMusic = new Set<number>();
    scanMusic(value, (index) => {
        if (index >= start && index < end) inMusic.add(index);
    });
    const segment = value.slice(start, end);
    let out = "";
    let last = 0;
    for (const match of segment.matchAll(PITCH_TOKEN)) {
        const at = start + (match.index ?? 0) + (match[1]?.length ?? 0);
        if (!inMusic.has(at)) continue;
        out += segment.slice(last, match.index) + crate().abc_shift(match[0], steps);
        last = (match.index ?? 0) + match[0].length;
    }
    return out + segment.slice(last);
}

/** Moves every written pitch in the selection (or the note at the caret)
 * by `steps` staff positions. */
function movePitches(steps: number): void {
    if (!wasm) return;
    const value = textarea.value;
    let start = textarea.selectionStart;
    let end = textarea.selectionEnd;
    if (start === end) {
        const anchor = anchorsInSelection()[0];
        if (anchor === undefined) return;
        start = anchor;
        end = tokenEnd(value, anchor);
    }
    const out = shiftedPitches(value, start, end, steps);
    if (out === value.slice(start, end)) return;
    applyEdit(start, end, out);
    textarea.setSelectionRange(start, start + out.length);
}

/** Moves the note at `anchor` by `steps` staff positions. */
function shiftNote(anchor: number, steps: number): void {
    if (!wasm) return;
    const value = textarea.value;
    const end = tokenEnd(value, anchor);
    applyEdit(anchor, end, shiftedPitches(value, anchor, end, steps));
}

/** Turns the note at `anchor` into a rest of the same length. */
function restAt(anchor: number): void {
    const value = textarea.value;
    const match = /^(\[[^\]]*\]|[\^_=]*[A-Ga-g][,']*)([0-9/]*)(-?)/.exec(value.slice(anchor));
    if (!match) return;
    refocusScore = true;
    applyEdit(anchor, anchor + match[0].length, `z${match[2]}`);
}

/** The string tuning chosen for the current tab view, lowest string first. */
function stringTuning(): string[] {
    const tablature = TABLATURES[viewSelect.value];
    if (!tablature) return [];
    return (tablature.tunings.find((tuning) => tuning.id === stringsSelect.value) ?? tablature.tunings[0]).notes;
}

/** Fills the string tuning menu for the current view and remembers the
 * choice per instrument. */
function renderStringTunings(): void {
    const tablature = TABLATURES[viewSelect.value];
    stringsSelect.hidden = !tablature || tablature.tunings.length < 2;
    if (!tablature) return;
    const stored = storedStrings()[viewSelect.value];
    stringsSelect.replaceChildren(
        ...tablature.tunings.map((tuning) => {
            const option = el("option", "", `Strings: ${tuning.name}`);
            option.value = tuning.id;
            return option;
        }),
    );
    stringsSelect.value = tablature.tunings.some((tuning) => tuning.id === stored) ? stored : tablature.tunings[0].id;
}

function storedStrings(): Record<string, string> {
    try {
        return JSON.parse(storageGet(STRINGS_KEY) ?? "{}") as Record<string, string>;
    } catch {
        return {};
    }
}

/** One tablature per staff of the drawn text: the instrument's, where the
 * crate finds every note written on the staff within its reach, and none
 * elsewhere. A bass tab under a
 * soprano line is thirty-odd frets up a neck that has twenty. */
function tablatureFor(text: string, tablature: (typeof TABLATURES)[string], tuning: string[]): object[] {
    const lines = ABCJS.parseOnly(text)[0]?.lines.filter((line) => line.staff?.length) ?? [];
    // A staff need not be on the first line: a voice that starts later, as
    // the generated chord part does, adds one only where it starts.
    const count = Math.max(0, ...lines.map((line) => line.staff?.length ?? 0));
    const staves = Array.from(
        { length: count },
        (_, index) => lines.find((line) => line.staff?.[index])?.staff?.[index] ?? { voices: [] },
    );
    tabSkipped = [];
    return staves.map((staff, index) => {
        const positions: number[] = [];
        let voices = 0;
        for (const line of lines) {
            const staffVoices = (line.staff?.[index]?.voices ?? []).filter((voice) =>
                voice.some((element) => element.pitches?.length),
            );
            voices = Math.max(voices, staffVoices.length);
            for (const voice of staffVoices) {
                for (const element of voice) {
                    for (const pitch of element.pitches ?? []) positions.push(pitch.pitch);
                }
            }
        }
        const name = staff.title ? [staff.title].flat().find(Boolean) : undefined;
        const which = name ? `“${name}”` : `staff ${index + 1}`;
        // abcjs tabs the first voice on a staff and leaves the rest out, so a
        // tab under two voices would pass off one line as the whole staff.
        if (voices > 1) {
            tabSkipped.push(`${which} holds ${voices} voices and tablature can show only one`);
            return { instrument: "" };
        }
        const clef = staff.clef?.type ?? "";
        const choice = crate().tab_tuning(tuning, clef, Int32Array.from(positions));
        if (!choice) {
            if (positions.length) tabSkipped.push(`${which} goes where the ${tablature.label.toLowerCase()} cannot reach`);
            return { instrument: "" };
        }
        return { instrument: tablature.instrument, label: tablature.label, tuning: choice };
    });
}

/** The chord symbols of the source realized as a part of their own, for a tab
 * view: each symbol voiced by the crate on the instrument's strings and held
 * until the next, split at the barlines. abcjs plays chord symbols but draws
 * no tablature for them, so without this a lead sheet's tab is its melody
 * alone. Null when the source has no chord symbols. */
function chordPart(source: string): { at: number; text: string }[] | null {
    const view = viewSelect.value;
    if (!chordsButton.classList.contains("on") || !wasm || !scoreInput || !analysis) return null;
    const tune = ABCJS.parseOnly(withoutOrnaments(source))[0];
    if (!tune) return null;
    const notes = notesByAnchor(scoreInput);
    const symbols = new Map<number, string>();
    for (const line of tune.lines) {
        for (const staff of line.staff ?? []) {
            for (const voice of staff.voices) {
                for (const element of voice) {
                    if (element.startChar == null || element.endChar == null) continue;
                    for (const chord of element.chord ?? []) {
                        if ((chord.position ?? "default") !== "default") continue;
                        const note = notes.get(anchorOf(source, element.startChar, element.endChar));
                        if (note && !symbols.has(note.start)) symbols.set(note.start, chord.name);
                    }
                }
            }
        }
    }
    if (symbols.size === 0) return null;

    // Guitar and bass are written an octave above where they sound. Without
    // a tab view to say otherwise, the chords are a guitar's.
    const octaveUp = view === "" || view === "guitar" || view === "bass";
    const strings = view === "" ? TABLATURES.guitar.tunings[0].notes : stringTuning();
    const clef = octaveUp ? (view === "bass" ? "bass-8" : "treble-8") : "treble";
    const symbolList = [...symbols].map(([start, name]) => ({ start, name }));
    const bars = wasm.chord_part(scoreInput, symbolList, strings, clef);

    const declaration = `V:tabchords clef=${clef} name="Chords"`;
    const additions: { at: number; text: string }[] = [];

    // abcjs lines voices up line by line, so the part is written a line of
    // its own under each line of music. That needs a voice for the music to
    // go back to, which a tune with voices of its own does not give as simply;
    // there the part follows the music and is laid out after it.
    const music: { start: number }[] = [];
    let inBody = false;
    let offset = 0;
    for (const line of source.split("\n")) {
        if (inBody && isMusicLine(line) && line.trim()) music.push({ start: offset });
        if (/^\s*K:/.test(line)) inBody = true;
        offset += line.length + 1;
    }
    if (voiceIds(source).length > 0 || music.length === 0) {
        additions.push({
            at: source.trimEnd().length,
            text: `\n${declaration}\n[L:1/8] ${bars.map((bar) => bar.abc).join(" | ")} |]`,
        });
        return additions;
    }
    const lineEnd = (start: number) => {
        const newline = source.indexOf("\n", start);
        return newline === -1 ? source.length : newline;
    };
    // A line's lyrics are the `w:` lines under it, so the part goes below
    // them; above them, abcjs would sing the words to the chords.
    const lyricsEnd = (start: number) => {
        let end = lineEnd(start);
        while (end < source.length && /^\s*(w:|\+:)/.test(source.slice(end + 1, lineEnd(end + 1)))) {
            end = lineEnd(end + 1);
        }
        return end;
    };
    const lineStarts = music.map((line) => {
        const end = lineEnd(line.start);
        const starts = [...notes.values()]
            .filter((note) => note.char_start >= line.start && note.char_start < end)
            .map((note) => note.start);
        return starts.length ? Math.min(...starts) : Infinity;
    });
    lineStarts[0] = 0;
    music.forEach((line, index) => {
        const from = lineStarts[index];
        const to = lineStarts.slice(index + 1).find((start) => start !== Infinity) ?? Infinity;
        const chunk = bars.filter((bar) => bar.start >= from - 1e-6 && bar.start < to - 1e-6).map((bar) => bar.abc);
        const last = index === music.length - 1;
        const voice = index === 0 ? `V:tabmusic\n` : `[V:tabmusic] `;
        additions.push({ at: line.start, text: voice });
        if (chunk.length === 0) return;
        const head = index === 0 ? `${declaration}\n` : `[V:tabchords] `;
        additions.push({
            at: lyricsEnd(line.start),
            text: `\n${head}[L:1/8] ${chunk.join(" | ")} ${last ? "|]" : "|"}`,
        });
    });
    return additions;
}

/** Which voice each line of the source belongs to: a `V:` field line or an
 * inline `[V:]` at its start names it, and a music line belongs to the
 * voice named last. Header lines other than `V:` belong to none. */
function lineVoices(source: string): { start: number; voice: string | null; directive: boolean }[] {
    const lines = [];
    const first = voiceIds(source)[0] ?? null;
    let inBody = false;
    let current: string | null = null;
    let start = 0;
    for (const line of source.split("\n")) {
        const field = /^\s*V:\s*([^\s\]]+)/.exec(line) ?? /^\s*\[V:\s*([^\s\]]+)\]/.exec(line);
        let voice: string | null = null;
        if (field) {
            voice = field[1];
            if (inBody) current = voice;
        } else if (/^\s*K:/.test(line)) {
            inBody = true;
        } else if (inBody && isMusicLine(line) && line.trim()) {
            voice = current ?? first;
        }
        lines.push({ start, voice, directive: /^\s*%%(score|staves)\b/.test(line) });
        start += line.length + 1;
    }
    return lines;
}

/** The starts of the lines to comment out so hidden parts are not drawn. */
function hiddenLineStarts(source: string): number[] {
    // A tab view lays every voice on a staff of its own, since abcjs tabs
    // only the first voice of a staff: the `%%score` line that groups them
    // goes, as it does when parts are hidden.
    if (hiddenParts.size === 0 && !TABLATURES[viewSelect.value]) return [];
    return lineVoices(source)
        .filter((line) => line.directive || (line.voice !== null && hiddenParts.has(line.voice)))
        .map((line) => line.start);
}

function renderParts(): void {
    const ids = voiceIds(textarea.value);
    for (const id of [...hiddenParts]) if (!ids.includes(id)) hiddenParts.delete(id);
    partsMenu.hidden = ids.length < 2;
    partsMenu.querySelector("summary")!.textContent = hiddenParts.size
        ? `Parts ${ids.length - hiddenParts.size}/${ids.length}`
        : "Parts";
    const names = analysis?.voices.length === ids.length ? analysis.voices.map((voice) => voice.name) : ids;
    const actions = el("div", "part-actions");
    const all = el("button", "", "All");
    all.type = "button";
    all.addEventListener("click", () => {
        hiddenParts.clear();
        redraw();
    });
    actions.append(all);
    partsNode.replaceChildren(
        actions,
        ...ids.map((id, index) => {
            const label = el("label");
            const box = el("input");
            box.type = "checkbox";
            box.checked = !hiddenParts.has(id);
            box.addEventListener("change", () => {
                if (box.checked) hiddenParts.delete(id);
                else if (hiddenParts.size < ids.length - 1) hiddenParts.add(id);
                else box.checked = true;
                redraw();
            });
            const name = names[index] && names[index] !== id ? `${names[index]} (${id})` : id;
            label.append(box, name);
            return label;
        }),
    );
}

function setZoom(next: number): void {
    zoom = next;
    zoomLevelButton.textContent = `${Math.round(zoom * 100)}%`;
    storageSet(ZOOM_KEY, String(zoom));
    redraw();
}

/** Draws the score again with the analysis already in hand. */
function redraw(): void {
    stopPlayback();
    render(textarea.value);
    renderParts();
    syncSelection();
}

/** Moves the fret numbers of the note at `anchor` across `strings` strings,
 * each keeping its fret: dropping a 3 from the A string onto the D string
 * makes the D string's third fret sound. */
function moveAcrossStrings(anchor: number, strings: number): boolean {
    if (!wasm || !rendered) return false;
    const note = notesByAnchor(scoreInput).get(anchor);
    if (!note) return false;
    const frets = (rendered.tune.getSelectableArray?.() ?? [])
        .filter((item) => item.absEl.abcelem.el_type === "tabNumber" && anchorOfElement(item.absEl.abcelem) === anchor)
        .flatMap((item) => Array.from(item.svgEl.querySelectorAll(".abcjs-tab-number")))
        .map((node) => Number(node.textContent))
        .filter((fret) => Number.isInteger(fret));
    if (!TABLATURES[viewSelect.value]) return false;
    const midis = Int32Array.from(note.pitches.map((pitch) => pitch.midi));
    const moved = wasm.move_frets(stringTuning(), note.clef, midis, Int32Array.from(frets), strings);
    if (!moved) return false;
    return setPitches(anchor, note, new Map(moved));
}

/** Rewrites some pitches of the note at `anchor` to sound new MIDI numbers,
 * spelled by the crate in the written key. */
function setPitches(anchor: number, note: NoteInput, moved: Map<number, number>): boolean {
    if (!wasm) return false;
    const value = textarea.value;
    const key = scoreInput?.key ?? null;
    const tokens = pitchTokens(value, anchor);
    const usedPitch = new Set<number>();
    const replacements: { start: number; end: number; text: string }[] = [];
    for (const token of tokens) {
        const index = note.pitches.findIndex((pitch, i) => !usedPitch.has(i) && pitch.staff === token.staff);
        if (index === -1) continue;
        usedPitch.add(index);
        const midi = moved.get(index);
        if (midi === undefined) continue;
        // Spell where it is written: a staff sounding an octave down is
        // written an octave up.
        const octaves = wasm.written_octaves(token.staff, note.pitches[index].midi);
        replacements.push({ start: token.start, end: token.end, text: wasm.abc_in_key(midi - 12 * octaves, key) });
    }
    if (replacements.length === 0) return false;
    const regionEnd = Math.max(...replacements.map((change) => change.end));
    let region = value.slice(anchor, regionEnd);
    for (const change of replacements.sort((a, b) => b.start - a.start)) {
        region = region.slice(0, change.start - anchor) + change.text + region.slice(change.end - anchor);
    }
    const midis = note.pitches.map((pitch, i) => moved.get(i) ?? pitch.midi);
    applyEdit(anchor, regionEnd, region, new Map([[anchor, midis]]));
    return true;
}

function insertAtCaret(text: string): void {
    const { selectionStart, selectionEnd, value } = textarea;
    if (text === "^" || text === "_" || text === "=") {
        // An accidental goes in front of the note at the caret.
        const anchor = anchorsInSelection()[0];
        if (anchor !== undefined) {
            const token = value.slice(anchor, tokenEnd(value, anchor));
            const bare = token.replace(/^[\^_=]+/, "");
            const current = token.slice(0, token.length - bare.length);
            const next = current === text ? "" : text;
            replaceRange(anchor, anchor + token.length, next + bare);
            textarea.setSelectionRange(anchor, anchor + next.length + bare.length);
            return;
        }
    }
    const pad = /^[\s|[]?$/.test(value.slice(selectionStart - 1, selectionStart)) || /^[/0-9-(]/.test(text) ? "" : " ";
    replaceRange(selectionStart, selectionEnd, pad + text);
}

// ------------------------------------------------------------- panels

function renderPanels(): void {
    const overview = $("#overview");
    const keys = $("#keys");
    const issues = $("#issues");
    const voices = $("#voices");
    const harmony = $("#harmony");
    const weights = $("#weights");
    const weightLabels = $("#weight-labels");
    if (!analysis) {
        for (const node of [overview, keys, issues, voices, harmony, weights, weightLabels]) node.replaceChildren();
        return;
    }
    const a = analysis;
    overview.replaceChildren(
        fact("Title", a.title || "Untitled"),
        fact("Written key", a.written_key ?? "none"),
        fact("Analysed in", a.analysis_key ?? "–"),
        fact("Meter", a.meter),
        fact("Tempo", `♩ = ${Math.round(a.tempo_bpm)}`),
        fact("Bars", String(a.measures)),
        fact("Notes", String(a.note_count)),
        fact("Length", `${Math.round(a.quarter_length * 100) / 100} quarters`),
    );

    keys.replaceChildren(
        ...a.key_estimates.map((estimate, index) =>
            fact(index === 0 ? "Best match" : `#${index + 1}`, `${estimate.key} (${estimate.score.toFixed(3)})`),
        ),
        fact("Tonal certainty", a.tonal_certainty.toFixed(3)),
    );
    const top = Math.max(...a.pitch_class_weights, 1e-9);
    const classNames = crate().pitch_class_names();
    weights.replaceChildren(
        ...a.pitch_class_weights.map((weight, index) => {
            const bar = el("div");
            bar.style.height = `${(weight / top) * 100}%`;
            bar.title = `${classNames[index]}: ${Math.round(weight * 100) / 100} quarters`;
            return bar;
        }),
    );
    weightLabels.replaceChildren(...classNames.map((name) => el("span", "", name)));

    $("#issue-count").textContent = a.issues.length ? `${a.issues.length} found` : "";
    if (a.issues.length === 0) {
        issues.replaceChildren(
            el(
                "p",
                "empty",
                a.voices.length > 1
                    ? "No parallel fifths or octaves, crossings or awkward leaps."
                    : "No awkward leaps. Write more voices (V:) to check the voice leading.",
            ),
        );
    } else {
        issues.replaceChildren(
            ...a.issues.map((issue) => {
                const button = el("button", `issue ${issue.severity}`);
                button.type = "button";
                const names = issue.voices.map((index) => a.voices[index]?.name ?? `Voice ${index + 1}`).join(" & ");
                button.append(
                    el("span", "badge", issue.severity === "warning" ? "fault" : "note"),
                    el("strong", "", `${issue.kind} · bar ${issue.measure}, beat ${formatBeat(issue.beat)}`),
                    el("small", "", `${names}: ${issue.detail}`),
                );
                button.addEventListener("click", () => {
                    const anchors = issue.chars.map((range) => range[0]);
                    focusAnchors(anchors);
                    const first = issue.chars[0];
                    if (first) {
                        revealInSource(first[0], tokenEnd(textarea.value, first[0]));
                        showMoment(first[0]);
                        focusAnchors(anchors);
                    }
                });
                return button;
            }),
        );
    }

    voices.replaceChildren(
        ...a.voices.map((voice) => {
            const row = el("tr");
            row.append(
                el("td", "", voice.name),
                el("td", "", String(voice.notes)),
                el("td", "", voice.lowest && voice.highest ? `${voice.lowest} – ${voice.highest}` : "–"),
                el("td", "", voice.range ?? "–"),
                el("td", "", voice.largest_leap ?? "–"),
            );
            return row;
        }),
    );

    harmony.replaceChildren(
        ...a.slices.map((slice) => {
            const row = el("tr", `row${slice.changed ? "" : " repeat"}`);
            row.append(
                el("td", "", String(slice.measure)),
                el("td", "", formatBeat(slice.beat)),
                el("td", "", slice.pitches.join(" ")),
                el("td", "", slice.pitched_common_name),
                el("td", "", slice.chord_symbol ?? ""),
                el("td", "", numeralOf(slice) ?? ""),
                el("td", "", slice.inversion == null ? "" : String(slice.inversion)),
            );
            row.addEventListener("click", () => {
                const anchors = slice.attacks.map((range) => range[0]);
                const first = slice.attacks[0] ?? slice.sounding[0];
                if (first) {
                    revealInSource(first[0], tokenEnd(textarea.value, first[0]));
                    showMoment(first[0]);
                }
                focusAnchors(anchors);
                scoreNode.scrollIntoView({ block: "nearest", behavior: "smooth" });
            });
            return row;
        }),
    );
}

// --------------------------------------------------------------- refresh

function refresh(): void {
    const source = textarea.value;
    storageSet(STORAGE_KEY, source);
    analysis = null;
    scoreInput = null;
    try {
        scoreInput = extract(source);
        if (wasm && scoreInput) analysis = wasm.analyze_score(scoreInput);
        clearError();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
    try {
        render(source);
    } catch (err) {
        fail(`abcjs could not engrave this: ${err instanceof Error ? err.message : String(err)}`);
    }
    renderPanels();
    renderParts();
    syncSelection();
}

function scheduleRefresh(delay = 250): void {
    window.clearTimeout(refreshTimer);
    refreshTimer = window.setTimeout(refresh, delay);
}

function loadText(text: string): void {
    stopPlayback();
    replaceRange(0, textarea.value.length, text);
    textarea.setSelectionRange(0, 0);
    textarea.scrollTop = 0;
    refresh();
}

// -------------------------------------------------------------- playback

function stopPlayback(): void {
    if (player) {
        player.timer.stop();
        player.synth.stop();
        player = null;
    }
    setClass("m21-playing", []);
    playButton.textContent = "Play";
    stopButton.disabled = true;
}

/** Keeps the note being played in view inside the score's own scroll. */
function followPlayback(node: SVGElement | undefined): void {
    if (!node) return;
    const box = node.getBoundingClientRect();
    const frame = scoreNode.getBoundingClientRect();
    if (box.top < frame.top + 40 || box.bottom > frame.bottom - 40) {
        scoreNode.scrollTo({ top: scoreNode.scrollTop + box.top - frame.top - frame.height / 3, behavior: "smooth" });
    }
}

/** A synth primed to play what is drawn in the chosen playback tuning. Where
 * a chord part plays the chord symbols, abcjs's own accompaniment of them is
 * switched off; `%%MIDI gchordoff` would have to be written inside a voice. */
async function makeSynth(drawn: Rendered): Promise<InstanceType<AbcjsGlobal["synth"]["CreateSynth"]>> {
    const synth = new ABCJS.synth.CreateSynth();
    const retune = tuningCallback();
    const options = { ...(retune ? { sequenceCallback: retune } : {}), ...(drawn.chordPart ? { chordsOff: true } : {}) };
    await synth.init({ visualObj: drawn.tune, options });
    await synth.prime();
    return synth;
}

/** The note the playback tuning is built on: the one chosen, or the tonic of
 * the key the score is analysed in. */
function tuningRoot(): string {
    return rootSelect.value || analysis?.analysis_key?.split(" ")[0] || "C";
}

/** A callback detuning every note abcjs is about to sound by the cents the
 * crate gives it, or null for equal temperament. A note is found by where
 * it is written, when it starts and its MIDI number; one the analysis never
 * saw (a grace note, a trill) takes the cents of its pitch class. */
function tuningCallback(): ((tracks: SynthNote[][]) => void) | null {
    const tuning = temperamentSelect.value;
    if (!tuning || !wasm || !scoreInput || !rendered) return null;
    const cents = wasm.score_tuning_cents(scoreInput, tuning, tuningRoot());
    const byNote = new Map<string, number>();
    scoreInput.voices.forEach((voice, v) =>
        voice.notes.forEach((note, n) =>
            note.pitches.forEach((pitch, p) => {
                byNote.set(`${note.char_start}|${Math.round(note.start * 1000)}|${pitch.midi}`, cents.notes[v][n][p]);
            }),
        ),
    );
    return (tracks) => {
        const lastByClass = new Map<number, number>();
        for (const track of tracks) {
            for (const note of track) {
                const anchor =
                    note.startChar != null && note.endChar != null
                        ? anchorOfElement({ el_type: "note", startChar: note.startChar, endChar: note.endChar })
                        : null;
                const pitchClass = ((note.pitch % 12) + 12) % 12;
                let detune =
                    anchor === null ? undefined : byNote.get(`${anchor}|${Math.round(note.start * 4000)}|${note.pitch}`);
                detune ??= cents.pitch_class_cents?.[pitchClass] ?? lastByClass.get(pitchClass);
                if (detune === undefined) continue;
                note.cents = detune;
                lastByClass.set(pitchClass, detune);
            }
        }
    };
}

/** Fills the playback tuning menu from the crate's tuning systems. */
function renderTemperaments(): void {
    if (!wasm) return;
    const stored = storageGet(TUNING_KEY) ?? "";
    const systems = wasm.playable_tuning_systems();
    const equal = el("option", "", "Tuning: equal temperament");
    equal.value = "";
    const adaptive = el("option", "", "Tuning: adaptive just intonation (follows the chords)");
    adaptive.value = ADAPTIVE_TUNING;
    adaptive.title = "Each note is tuned five-limit just against the root of the chord sounding when it starts.";
    temperamentSelect.replaceChildren(
        equal,
        adaptive,
        ...systems
            .filter((system) => system.id !== "EqualTemperament")
            .map((system) => {
                const option = el("option", "", `Tuning: ${system.name}`);
                option.value = system.id;
                option.title = system.description;
                return option;
            }),
    );
    temperamentSelect.value = Array.from(temperamentSelect.options).some((option) => option.value === stored) ? stored : "";
    rootSelect.replaceChildren(
        ...["", ...wasm.pitch_class_names()].map((name) => {
            const option = el("option", "", name ? `Root: ${name.replace("-", "♭").replace("#", "♯")}` : "Root: key tonic");
            option.value = name;
            return option;
        }),
    );
    rootSelect.value = storageGet(ROOT_KEY) ?? "";
    rootSelect.hidden = !temperamentSelect.value;
}

async function play(): Promise<void> {
    if (!rendered) return;
    if (!ABCJS.synth.supportsAudio()) {
        fail("Audio is not available in this browser.");
        return;
    }
    stopPlayback();
    playButton.disabled = true;
    playButton.textContent = "Loading…";
    try {
        const synth = await makeSynth(rendered);
        const timer = new ABCJS.TimingCallbacks(rendered.tune, {
            eventCallback: (event) => {
                scoreNode.querySelectorAll(".m21-playing").forEach((node) => node.classList.remove("m21-playing"));
                if (!event) {
                    stopPlayback();
                    if (loopButton.classList.contains("on")) void play();
                    return;
                }
                for (const group of event.elements ?? []) for (const node of group) node.classList.add("m21-playing");
                followPlayback(event.elements?.[0]?.[0]);
            },
        });
        player = { synth, timer };
        synth.start();
        timer.start();
        playButton.textContent = "Playing";
        stopButton.disabled = false;
    } catch (err) {
        fail(`Playback failed: ${err instanceof Error ? err.message : String(err)}`);
        stopPlayback();
    } finally {
        playButton.disabled = false;
    }
}

// --------------------------------------------------------------- exports

function scoreSvg(): SVGSVGElement | null {
    const svg = scoreNode.querySelector("svg");
    if (!svg) return null;
    const copy = svg.cloneNode(true) as SVGSVGElement;
    const box = svg.viewBox.baseVal;
    const width = box && box.width ? box.width : svg.getBoundingClientRect().width;
    const height = box && box.height ? box.height : svg.getBoundingClientRect().height;
    copy.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    copy.setAttribute("width", String(Math.ceil(width)));
    copy.setAttribute("height", String(Math.ceil(height)));
    copy.removeAttribute("style");
    copy.setAttribute("style", "color:#000;background:#fff");
    copy.querySelectorAll(".m21-focus,.m21-flag,.m21-playing").forEach((node) => {
        node.classList.remove("m21-focus", "m21-flag", "m21-playing");
    });
    const background = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    background.setAttribute("width", "100%");
    background.setAttribute("height", "100%");
    background.setAttribute("fill", "#fff");
    copy.insertBefore(background, copy.firstChild);
    return copy;
}

async function exportAs(kind: string): Promise<void> {
    exportMenu.open = false;
    const stem = fileStem();
    try {
        switch (kind) {
            case "midi":
            case "musicxml": {
                if (!wasm || !scoreInput || scoreInput.voices.length === 0) throw new Error("There are no notes to export.");
                if (kind === "midi") download(`${stem}.mid`, wasm.score_to_midi(scoreInput) as Uint8Array<ArrayBuffer>, "audio/midi");
                else download(`${stem}.musicxml`, wasm.score_to_musicxml(scoreInput), "application/vnd.recordare.musicxml+xml");
                break;
            }
            case "abc":
                download(`${stem}.abc`, textarea.value, "text/vnd.abc");
                break;
            case "abc-analysis":
                download(`${stem}-analysis.abc`, rendered?.text ?? textarea.value, "text/vnd.abc");
                break;
            case "svg": {
                const svg = scoreSvg();
                if (!svg) throw new Error("There is no score to export.");
                download(`${stem}.svg`, new XMLSerializer().serializeToString(svg), "image/svg+xml");
                break;
            }
            case "png": {
                const svg = scoreSvg();
                if (!svg) throw new Error("There is no score to export.");
                const width = Number(svg.getAttribute("width"));
                const height = Number(svg.getAttribute("height"));
                const url = URL.createObjectURL(
                    new Blob([new XMLSerializer().serializeToString(svg)], { type: "image/svg+xml" }),
                );
                const image = new Image();
                await new Promise<void>((resolve, reject) => {
                    image.onload = () => resolve();
                    image.onerror = () => reject(new Error("The score could not be drawn."));
                    image.src = url;
                });
                const canvas = el("canvas");
                canvas.width = width * 2;
                canvas.height = height * 2;
                const context = canvas.getContext("2d");
                if (!context) throw new Error("Canvas is not available.");
                context.scale(2, 2);
                context.drawImage(image, 0, 0, width, height);
                URL.revokeObjectURL(url);
                const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
                if (!blob) throw new Error("The PNG could not be written.");
                download(`${stem}.png`, blob, "image/png");
                break;
            }
            case "wav": {
                if (!rendered) throw new Error("There is no score to export.");
                const synth = await makeSynth(rendered);
                const link = el("a");
                link.href = synth.download();
                link.download = `${stem}.wav`;
                link.click();
                break;
            }
            case "json":
                download(`${stem}-analysis.json`, JSON.stringify(analysis, null, 2), "application/json");
                break;
            case "csv": {
                const quote = (value: string | number | null) => `"${String(value ?? "").replace(/"/g, '""')}"`;
                const rows = [
                    ["bar", "beat", "offset", "pitches", "chord", "symbol", "numeral", "music21 numeral", "inversion"].join(","),
                    ...(analysis?.slices ?? []).map((slice) =>
                        [
                            slice.measure,
                            formatBeat(slice.beat),
                            slice.offset,
                            slice.pitches.join(" "),
                            slice.pitched_common_name,
                            slice.chord_symbol,
                            slice.textbook_numeral,
                            slice.numeral,
                            slice.inversion,
                        ]
                            .map(quote)
                            .join(","),
                    ),
                ];
                download(`${stem}-harmony.csv`, rows.join("\n"), "text/csv");
                break;
            }
            case "print":
                window.print();
                break;
        }
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

// ----------------------------------------------------------------- sharing

function toBase64Url(bytes: Uint8Array): string {
    let binary = "";
    for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromBase64Url(text: string): Uint8Array {
    const binary = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
    return Uint8Array.from(binary, (ch) => ch.charCodeAt(0));
}

async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
    const piped = new Blob([bytes as Uint8Array<ArrayBuffer>]).stream().pipeThrough(stream);
    return new Uint8Array(await new Response(piped).arrayBuffer());
}

/** The source packed for a URL: deflated where the browser can, marked `z`,
 * and otherwise plain UTF-8, marked `t`. Either way base64url. */
async function packScore(abc: string): Promise<string> {
    const bytes = new TextEncoder().encode(abc);
    if (typeof CompressionStream !== "undefined") {
        return `z${toBase64Url(await pipe(bytes, new CompressionStream("deflate-raw")))}`;
    }
    return `t${toBase64Url(bytes)}`;
}

async function unpackScore(packed: string): Promise<string> {
    const bytes = fromBase64Url(packed.slice(1));
    if (packed.startsWith("z")) return new TextDecoder().decode(await pipe(bytes, new DecompressionStream("deflate-raw")));
    return new TextDecoder().decode(bytes);
}

async function shareLink(): Promise<string> {
    const params = new URLSearchParams({ abc: await packScore(textarea.value) });
    if (viewSelect.value) params.set("view", viewSelect.value);
    if (viewSelect.value && stringsSelect.value && stringsSelect.value !== "standard") params.set("strings", stringsSelect.value);
    if (temperamentSelect.value) params.set("tuning", temperamentSelect.value);
    if (temperamentSelect.value && rootSelect.value) params.set("root", rootSelect.value);
    if (labelsSelect.value !== "numeral") params.set("labels", labelsSelect.value);
    if (numeralsSelect.value !== "textbook") params.set("numerals", numeralsSelect.value);
    const url = new URL(window.location.href);
    url.search = "";
    url.hash = params.toString();
    return url.href;
}

async function openShare(): Promise<void> {
    const link = await shareLink();
    shareUrl.value = link;
    shareNote.textContent =
        link.length > 8000
            ? `This link is ${link.length.toLocaleString()} characters; some apps cut long links short, so export the ABC for a score this size.`
            : `${link.length.toLocaleString()} characters.`;
    $<HTMLButtonElement>("#share-send").hidden = typeof navigator.share !== "function";
    shareUrl.focus();
    shareUrl.select();
}

/** Loads the score a shared link carries in its hash, then clears the hash
 * so later edits do not leave a stale link in the address bar. */
async function loadFromHash(): Promise<boolean> {
    const params = new URLSearchParams(window.location.hash.slice(1));
    const packed = params.get("abc");
    if (!packed) return false;
    try {
        textarea.value = await unpackScore(packed);
        const view = params.get("view");
        if (view !== null && view in TABLATURES) viewSelect.value = view;
        const strings = params.get("strings");
        if (view && strings) storageSet(STRINGS_KEY, JSON.stringify({ ...storedStrings(), [view]: strings }));
        const tuning = params.get("tuning");
        if (tuning !== null) storageSet(TUNING_KEY, tuning);
        storageSet(ROOT_KEY, params.get("root") ?? "");
        const labels = params.get("labels");
        if (labels && Array.from(labelsSelect.options).some((option) => option.value === labels)) labelsSelect.value = labels;
        numeralsSelect.hidden = labelsSelect.value !== "numeral";
        const numerals = params.get("numerals");
        if (numerals && Array.from(numeralsSelect.options).some((option) => option.value === numerals)) {
            numeralsSelect.value = numerals;
        }
        history.replaceState(null, "", window.location.pathname + window.location.search);
        return true;
    } catch {
        fail("This share link is damaged, so the score it carried could not be read.");
        return false;
    }
}

// ----------------------------------------------------------------- wiring

function renderExamples(): void {
    const container = $("#examples");
    for (const example of EXAMPLES) {
        const chip = el("button", "chip", example.name);
        chip.type = "button";
        chip.addEventListener("click", () => {
            if (example.view !== undefined && example.view !== viewSelect.value) {
                viewSelect.value = example.view;
                storageSet(VIEW_KEY, viewSelect.value);
                renderStringTunings();
            }
            loadText(example.abc);
        });
        container.append(chip);
    }
}

textarea.addEventListener("input", () => scheduleRefresh());
textarea.addEventListener("keyup", syncSelection);
textarea.addEventListener("click", syncSelection);
textarea.addEventListener("keydown", (event) => {
    if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
        event.preventDefault();
        const direction = event.key === "ArrowUp" ? 1 : -1;
        movePitches(direction * (event.shiftKey ? 7 : 1));
    } else if (event.key === "Tab" && !event.shiftKey) {
        event.preventDefault();
        replaceRange(textarea.selectionStart, textarea.selectionEnd, "    ");
    }
});

// The score takes the arrow keys and Delete for the note selected in it.
// Arrow keys are caught before abcjs sees them, since abcjs only finishes a
// keyboard drag on Enter.
scoreNode.addEventListener(
    "keydown",
    (event) => {
        if (!selected || !(event.target instanceof Element) || !event.target.closest("[selectable]")) return;
        const current = selected;
        if (event.key === "ArrowUp" || event.key === "ArrowDown") {
            event.preventDefault();
            event.stopPropagation();
            const direction = event.key === "ArrowUp" ? 1 : -1;
            refocusScore = true;
            if (current.tab) {
                if (!moveAcrossStrings(current.anchor, direction)) refocusScore = false;
            } else {
                shiftNote(current.anchor, direction * (event.shiftKey ? 7 : 1));
            }
        } else if (event.key === "Delete" || event.key === "Backspace") {
            event.preventDefault();
            event.stopPropagation();
            restAt(current.anchor);
        }
    },
    true,
);
scoreNode.addEventListener(
    "keyup",
    (event) => {
        if (event.key === "ArrowUp" || event.key === "ArrowDown") event.stopPropagation();
    },
    true,
);

loopButton.addEventListener("click", () => loopButton.classList.toggle("on"));

$("#palette").addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest("button");
    if (!button) return;
    if (button.dataset.move) movePitches(Number(button.dataset.move));
    else if (button.dataset.insert) insertAtCaret(button.dataset.insert);
});

viewSelect.addEventListener("change", () => {
    storageSet(VIEW_KEY, viewSelect.value);
    renderStringTunings();
    redraw();
});

stringsSelect.addEventListener("change", () => {
    storageSet(STRINGS_KEY, JSON.stringify({ ...storedStrings(), [viewSelect.value]: stringsSelect.value }));
    redraw();
});

temperamentSelect.addEventListener("change", () => {
    storageSet(TUNING_KEY, temperamentSelect.value);
    rootSelect.hidden = !temperamentSelect.value;
    stopPlayback();
});

rootSelect.addEventListener("change", () => {
    storageSet(ROOT_KEY, rootSelect.value);
    stopPlayback();
});

$("#zoom-out").addEventListener("click", () => setZoom(ZOOMS[Math.max(0, ZOOMS.indexOf(zoom) - 1)] ?? 1));
$("#zoom-in").addEventListener("click", () => setZoom(ZOOMS[Math.min(ZOOMS.length - 1, ZOOMS.indexOf(zoom) + 1)] ?? 1));
zoomLevelButton.addEventListener("click", () => setZoom(1));
wideButton.addEventListener("click", () => {
    const wide = workspace.classList.toggle("wide");
    wideButton.classList.toggle("on", wide);
    storageSet(WIDE_KEY, wide ? "1" : "");
});
// The bars are laid out to the score's width, so a new width is a new layout.
let lastWidth = 0;
let resizeTimer = 0;
new ResizeObserver(() => {
    if (!rendered || Math.abs(scoreNode.clientWidth - lastWidth) < 8) return;
    lastWidth = scoreNode.clientWidth;
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
        render(textarea.value);
        syncSelection();
    }, 150);
}).observe(scoreNode);
document.addEventListener("click", (event) => {
    if (partsMenu.open && !partsMenu.contains(event.target as Node)) partsMenu.open = false;
});

shareMenu.addEventListener("toggle", () => {
    if (!shareMenu.open) return;
    exportMenu.open = false;
    void openShare();
});
$("#share-copy").addEventListener("click", async () => {
    const button = $<HTMLButtonElement>("#share-copy");
    try {
        await navigator.clipboard.writeText(shareUrl.value);
        button.textContent = "Copied";
    } catch {
        shareUrl.select();
        button.textContent = "Press Ctrl+C";
    }
    setTimeout(() => (button.textContent = "Copy link"), 1800);
});
$("#share-send").addEventListener("click", () => {
    navigator.share({ title: analysis?.title || "Score", url: shareUrl.value }).catch(() => undefined);
});

labelsSelect.addEventListener("change", () => {
    storageSet(LABELS_KEY, labelsSelect.value);
    numeralsSelect.hidden = labelsSelect.value !== "numeral";
    render(textarea.value);
    syncSelection();
});

numeralsSelect.addEventListener("change", () => {
    storageSet(NUMERALS_KEY, numeralsSelect.value);
    render(textarea.value);
    syncSelection();
    renderPanels();
});

flagsButton.addEventListener("click", () => {
    flagsButton.classList.toggle("on");
    markIssues();
});

ringButton.addEventListener("click", () => {
    storageSet(RING_KEY, ringButton.classList.toggle("on") ? "" : "off");
    stopPlayback();
    refresh();
});

chordsButton.addEventListener("click", () => {
    storageSet(CHORDS_KEY, chordsButton.classList.toggle("on") ? "" : "off");
    redraw();
});

for (const [id, steps] of [
    ["#up", 1],
    ["#down", -1],
] as const) {
    $(id).addEventListener("click", () => {
        const source = textarea.value;
        try {
            const moved = ABCJS.strTranspose(source, ABCJS.parseOnly(source), steps);
            if (moved !== source) loadText(moved);
        } catch (err) {
            fail(`Transposition failed: ${err instanceof Error ? err.message : String(err)}`);
        }
    });
}

playButton.addEventListener("click", () => {
    if (player) stopPlayback();
    else void play();
});
stopButton.addEventListener("click", stopPlayback);

$("#open-file").addEventListener("change", async (event) => {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    try {
        if (/\.midi?$/i.test(file.name) || file.type === "audio/midi") {
            if (!wasm) throw new Error("music21-rs has not loaded yet.");
            loadText(wasm.midi_to_abc(new Uint8Array(await file.arrayBuffer())));
        } else {
            loadText(await file.text());
        }
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
});

exportMenu.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-export]");
    if (button?.dataset.export) void exportAs(button.dataset.export);
});
document.addEventListener("click", (event) => {
    for (const menu of [exportMenu, shareMenu]) {
        if (menu.open && !menu.contains(event.target as Node)) menu.open = false;
    }
});
exportMenu.addEventListener("toggle", () => {
    if (exportMenu.open) shareMenu.open = false;
});

async function start(): Promise<void> {
    renderExamples();
    labelsSelect.value = storageGet(LABELS_KEY) ?? "numeral";
    if (!labelsSelect.value) labelsSelect.value = "numeral";
    numeralsSelect.value = storageGet(NUMERALS_KEY) ?? "textbook";
    if (!numeralsSelect.value) numeralsSelect.value = "textbook";
    numeralsSelect.hidden = labelsSelect.value !== "numeral";
    const storedZoom = Number(storageGet(ZOOM_KEY));
    zoom = ZOOMS.includes(storedZoom) ? storedZoom : 1;
    zoomLevelButton.textContent = `${Math.round(zoom * 100)}%`;
    if (storageGet(WIDE_KEY)) {
        workspace.classList.add("wide");
        wideButton.classList.add("on");
    }
    ringButton.classList.toggle("on", storageGet(RING_KEY) !== "off");
    chordsButton.classList.toggle("on", storageGet(CHORDS_KEY) !== "off");
    viewSelect.value = storageGet(VIEW_KEY) ?? "";
    if (!(await loadFromHash())) textarea.value = storageGet(STORAGE_KEY) ?? EXAMPLES[0].abc;
    if (!(viewSelect.value in TABLATURES)) viewSelect.value = "";
    renderStringTunings();
    if (typeof ABCJS === "undefined") {
        fail("abcjs could not be loaded from the CDN, so the score cannot be engraved.");
        return;
    }
    refresh();
    try {
        const module = (await import(new URL("../pkg/music21_rs_web.js", window.location.href).href)) as WasmModule;
        await module.default();
        wasm = module;
        renderTemperaments();
        playButton.disabled = false;
        refresh();
    } catch (err) {
        fail(err instanceof Error ? err.message : String(err));
    }
}

void start();
