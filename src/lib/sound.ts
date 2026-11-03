// Sound for the notebook's rewards, synthesised with WebAudio (no audio files to ship).
// Sound here is information, not decoration:
//   · winning a gate plays a rising arpeggio with MORE notes the sooner you passed
//     (first try = 4 notes, second = 3, persistence = 2), a sticker is a bell;
//   · bricks click with a plastic "clack" whose loudness follows the impact;
//   · a sticker peels with a paper "shff" and sticks with a soft tap.
// It is on by default at a modest volume, can be muted (remembered on this device), never
// plays before the student has interacted with the page, and rate-limits itself.

import { create } from "zustand";
import type { Piece } from "./rewards";

// --- Pure rules ---------------------------------------------------------------------------------

export interface Note {
  /** Hz. */
  freq: number;
  /** Seconds after the start. */
  at: number;
  dur: number;
  type: OscillatorType;
}

const C5 = 523.25;
const E5 = 659.25;
const G5 = 783.99;
const C6 = 1046.5;
const A5 = 880;
const E6 = 1318.5;

/** The notes a piece plays when it is earned. More notes = it was earned sooner. */
export function earnNotes(piece: Pick<Piece, "shape" | "tone">): Note[] {
  if (piece.shape !== "brick") {
    // A sticker: two bright bell tones.
    return [
      { freq: A5, at: 0, dur: 0.28, type: "sine" },
      { freq: E6, at: 0.09, dur: 0.4, type: "sine" },
    ];
  }
  const ladder = piece.tone === "gold" ? [C5, E5, G5, C6] : piece.tone === "blue" ? [C5, E5, G5] : [C5, E5];
  return ladder.map((freq, i) => ({ freq, at: i * 0.085, dur: 0.22 + i * 0.03, type: "triangle" }));
}

/** Loudness (0..1) of an impact at `speed` px/frame. Below the floor it makes no sound at all
 *  (pieces resting against each other must not rattle). */
export function impactGain(speed: number): number {
  const FLOOR = 1.4;
  const FULL = 9;
  if (!(speed > FLOOR)) return 0;
  return Math.min(1, ((speed - FLOOR) / (FULL - FLOOR)) ** 0.8) * 0.9 + 0.1;
}

/** Pitch of a clack by the width of the piece: small pieces tick high, big ones knock low. */
export function clackPitch(width: number): number {
  return 2600 - Math.min(1400, Math.max(0, width - 30) * 13);
}

/** Rate limit: a sound may play only if `minGap` ms have passed since the last one of its kind. */
export function mayPlay(now: number, last: number, minGap: number): boolean {
  return now - last >= minGap;
}

// --- State: on / off, remembered ---------------------------------------------------------------------

const KEY = "learnkit.sound";

function readOn(): boolean {
  try {
    return window.localStorage.getItem(KEY) !== "off";
  } catch {
    return true;
  }
}

export const useSound = create<{ on: boolean; toggle: () => void }>((set, get) => ({
  on: readOn(),
  toggle: () => {
    const on = !get().on;
    try {
      window.localStorage.setItem(KEY, on ? "on" : "off");
    } catch {
      /* private window: the choice lasts for the session */
    }
    set({ on });
  },
}));

// --- Engine -------------------------------------------------------------------------------------------

const MASTER = 0.32;
let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let noise: AudioBuffer | null = null;
const lastAt: Record<string, number> = {};

/** The context, created on first use. Returns null when sound is off, unsupported, or the page has
 *  not been interacted with yet (the browser would keep it suspended). */
function audio(): AudioContext | null {
  if (!useSound.getState().on || typeof window === "undefined") return null;
  const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return null;
  try {
    if (!ctx) {
      ctx = new Ctor();
      master = ctx.createGain();
      master.gain.value = MASTER;
      master.connect(ctx.destination);
    }
    if (ctx.state === "suspended") void ctx.resume();
    return ctx.state === "running" || ctx.state === "suspended" ? ctx : null;
  } catch {
    return null;
  }
}

function noiseBuffer(c: AudioContext): AudioBuffer {
  if (noise) return noise;
  const len = Math.floor(c.sampleRate * 0.5);
  noise = c.createBuffer(1, len, c.sampleRate);
  const data = noise.getChannelData(0);
  for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;
  return noise;
}

function tone(c: AudioContext, n: Note, gain: number, t0: number) {
  const osc = c.createOscillator();
  const g = c.createGain();
  osc.type = n.type;
  osc.frequency.value = n.freq;
  const start = t0 + n.at;
  g.gain.setValueAtTime(0.0001, start);
  g.gain.exponentialRampToValueAtTime(gain, start + 0.012);
  g.gain.exponentialRampToValueAtTime(0.0001, start + n.dur);
  osc.connect(g).connect(master!);
  osc.start(start);
  osc.stop(start + n.dur + 0.05);
}

function burst(c: AudioContext, opts: { dur: number; freq: number; q: number; gain: number; type?: BiquadFilterType; sweepTo?: number; at?: number }) {
  const src = c.createBufferSource();
  src.buffer = noiseBuffer(c);
  const f = c.createBiquadFilter();
  f.type = opts.type ?? "bandpass";
  const start = c.currentTime + (opts.at ?? 0);
  f.frequency.setValueAtTime(opts.freq, start);
  if (opts.sweepTo) f.frequency.exponentialRampToValueAtTime(opts.sweepTo, start + opts.dur);
  f.Q.value = opts.q;
  const g = c.createGain();
  g.gain.setValueAtTime(opts.gain, start);
  g.gain.exponentialRampToValueAtTime(0.0001, start + opts.dur);
  src.connect(f).connect(g).connect(master!);
  src.start(start, Math.random() * 0.2);
  src.stop(start + opts.dur + 0.02);
}

function limited(kind: string, gap: number): boolean {
  const now = performance.now();
  if (!mayPlay(now, lastAt[kind] ?? -Infinity, gap)) return false;
  lastAt[kind] = now;
  return true;
}

export const sfx = {
  /** A piece was earned. */
  earn(piece: Pick<Piece, "shape" | "tone">) {
    const c = audio();
    if (!c || !limited("earn", 250)) return;
    const t0 = c.currentTime + 0.02;
    for (const n of earnNotes(piece)) tone(c, n, 0.5, t0);
  },
  /** Two pieces (or a piece and the plate) hit at `speed`; `width` is the piece's width in px. */
  clack(speed: number, width: number) {
    const gain = impactGain(speed);
    if (gain === 0) return;
    const c = audio();
    if (!c || !limited("clack", 45)) return;
    burst(c, { dur: 0.05, freq: clackPitch(width), q: 3.5, gain: 0.75 * gain });
    tone(c, { freq: 150 - Math.min(60, width / 3), at: 0, dur: 0.07, type: "sine" }, 0.5 * gain, c.currentTime);
  },
  /** A sticker starts to peel: a short paper "shff". */
  peel() {
    const c = audio();
    if (!c || !limited("peel", 220)) return;
    burst(c, { dur: 0.2, freq: 2200, sweepTo: 6200, q: 0.7, gain: 0.32, type: "highpass" });
  },
  /** A sticker is stuck down: a soft tap. */
  stick() {
    const c = audio();
    if (!c || !limited("stick", 120)) return;
    burst(c, { dur: 0.05, freq: 900, q: 1.2, gain: 0.5 });
    tone(c, { freq: 220, at: 0, dur: 0.09, type: "sine" }, 0.35, c.currentTime);
  },
  /** A piece was picked up. */
  pick() {
    const c = audio();
    if (!c || !limited("pick", 80)) return;
    tone(c, { freq: 620, at: 0, dur: 0.06, type: "sine" }, 0.22, c.currentTime);
  },
};
