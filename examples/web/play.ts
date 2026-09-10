/// Plays pitches through the Web Audio API: what the smaller pages need to
/// sound a chord or a scale without carrying a synth of their own.

let context: AudioContext | null = null;
let active: OscillatorNode[] = [];

async function ensureContext(): Promise<AudioContext | null> {
    const Constructor =
        window.AudioContext ||
        (window as unknown as { webkitAudioContext?: typeof AudioContext })
            .webkitAudioContext;
    if (!Constructor) return null;
    context ||= new Constructor();
    if (context.state === "suspended") await context.resume();
    return context;
}

/// The frequency of a MIDI note in twelve-tone equal temperament at A440.
export function midiToHz(midi: number): number {
    return 440 * Math.pow(2, (midi - 69) / 12);
}

function tone(
    audio: AudioContext,
    frequency: number,
    at: number,
    duration: number,
    level: number,
): void {
    const oscillator = audio.createOscillator();
    const gain = audio.createGain();
    oscillator.type = "triangle";
    oscillator.frequency.setValueAtTime(frequency, at);
    gain.gain.setValueAtTime(0.0001, at);
    gain.gain.exponentialRampToValueAtTime(level, at + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + duration);
    oscillator.connect(gain).connect(audio.destination);
    oscillator.start(at);
    oscillator.stop(at + duration + 0.05);
    active.push(oscillator);
    oscillator.addEventListener("ended", () => {
        active = active.filter((node) => node !== oscillator);
    });
}

/// Sounds frequencies together, then one after another when `arpeggio`.
export async function play(
    frequencies: number[],
    { arpeggio = false, step = 0.4, hold = 1.4 } = {},
): Promise<boolean> {
    const audio = await ensureContext();
    if (!audio) return false;
    stop();
    const start = audio.currentTime + 0.03;
    const level = Math.max(0.06, 0.32 / Math.sqrt(Math.max(1, frequencies.length)));
    frequencies.forEach((frequency, index) => {
        if (!(frequency > 0)) return;
        if (arpeggio) {
            tone(audio, frequency, start + index * step, step * 0.92, 0.28);
        } else {
            tone(audio, frequency, start, hold, level);
        }
    });
    return true;
}

export function stop(): void {
    for (const node of active) {
        try {
            node.stop();
        } catch {
            // Already stopped.
        }
    }
    active = [];
}
