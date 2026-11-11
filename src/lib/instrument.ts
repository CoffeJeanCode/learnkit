// Guitar data for the `instrument` engine — mirror of
// `src-tauri/src/domain/instrument.rs`. Shapes are low E → high e, "x" = muted.

export const GUITAR_CHORDS: Record<string, string> = {
  C: "x32010",
  D: "xx0232",
  E: "022100",
  G: "320003",
  A: "x02220",
  F: "133211",
  Am: "x02210",
  Em: "022000",
  Dm: "xx0231",
  E7: "020100",
  A7: "x02020",
  D7: "xx0212",
  G7: "320001",
  C7: "x32310",
  B7: "x21202",
  Cmaj7: "x32000",
  Fmaj7: "xx3210",
  Asus2: "x02200",
  Dsus4: "xx0233",
};

/** Open-string MIDI notes, low E → high e (E2 A2 D3 G3 B3 E4). */
export const OPEN_STRINGS = [40, 45, 50, 55, 59, 64];

/** Frets per string (null = muted). */
export function shapeOf(chord: string): Array<number | null> | null {
  const s = GUITAR_CHORDS[chord.trim()];
  return s ? [...s].map((c) => (c === "x" ? null : Number(c))) : null;
}

/** Frequencies (Hz) the chord sounds, low → high. */
export function chordFrequencies(chord: string): number[] {
  const shape = shapeOf(chord);
  if (!shape) return [];
  return shape.flatMap((fret, i) => (fret === null ? [] : [440 * Math.pow(2, (OPEN_STRINGS[i] + fret - 69) / 12)]));
}

export interface StrumEvent {
  /** Seconds from the start. */
  at: number;
  chord: string;
  dir: "D" | "U";
  /** Index of the bar being played. */
  bar: number;
}

/** Every strum of a progression at `bpm`: the pattern spreads over 4 beats
 *  and repeats for each bar's length. */
export function schedule(progression: Array<{ chord: string; beats: number }>, strum: string | null | undefined, bpm: number): { events: StrumEvent[]; total: number } {
  const pattern = strum && /^[DU-]{4,16}$/.test(strum) ? strum : "D---D---D---D---".slice(0, 4);
  const beat = 60 / bpm;
  const slot = (4 * beat) / pattern.length;
  const events: StrumEvent[] = [];
  let t = 0;
  progression.forEach((bar, b) => {
    const slots = Math.round((bar.beats / 4) * pattern.length);
    for (let i = 0; i < slots; i++) {
      const c = pattern[i % pattern.length];
      if (c === "D" || c === "U") events.push({ at: t + i * slot, chord: bar.chord, dir: c, bar: b });
    }
    t += bar.beats * beat;
  });
  return { events, total: t };
}
