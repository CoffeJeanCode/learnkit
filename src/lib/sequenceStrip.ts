// Pure logic of the `sequence_strip` engine: how a chunk splits into symbols
// and what changed between a row and the first one. The model only writes
// the rows; the difference is computed here, so it is always exact.

export interface ChunkLike {
  text: string;
  meaning: string;
}

/** Letters, unless the chunk holds words ("GET /index"): then words. */
export function symbolsOf(text: string): string[] {
  const t = text.trim();
  return /\s/.test(t) ? t.split(/\s+/) : Array.from(t);
}

export interface ChunkDiff {
  /** Indices of the symbols that differ from the same chunk of the first row. */
  changedSymbols: Set<number>;
  /** The chunk now means something else. */
  meaningChanged: boolean;
}

const norm = (s: string) => s.trim().toLowerCase();

/** Per-chunk difference of `row` against `base` (position by position). */
export function diffRow(base: ChunkLike[], row: ChunkLike[]): ChunkDiff[] {
  return row.map((chunk, i) => {
    const ref = base[i];
    if (!ref) return { changedSymbols: new Set(symbolsOf(chunk.text).map((_, k) => k)), meaningChanged: true };
    const a = symbolsOf(ref.text);
    const b = symbolsOf(chunk.text);
    const changedSymbols = new Set<number>();
    b.forEach((s, k) => {
      if (a[k] !== s) changedSymbols.add(k);
    });
    return { changedSymbols, meaningChanged: norm(ref.meaning) !== norm(chunk.meaning) };
  });
}

const PLACEHOLDER_WORDS = new Set(["palabra", "paso", "elemento", "grupo", "parte", "item", "ítem", "pieza", "cosa", "bloque", "nodo"]);

/** "palabra 1", "¿palabra 2?", "paso 3 · paso 4" — numbered placeholders that
 *  name nothing (mirrors `is_placeholder_label` in Rust). */
export function isPlaceholderLabel(label: string): boolean {
  const cleaned = label.replace(/[()¿?[\]]/g, " ");
  const pieces = cleaned
    .split(/[·,;/|\-→]/)
    .map((p) => p.trim())
    .filter(Boolean);
  return (
    pieces.length > 0 &&
    pieces.every((p) => {
      const words = p.split(/\s+/);
      return words.length === 2 && PLACEHOLDER_WORDS.has(words[0].toLowerCase()) && /^\d+$/.test(words[1]);
    })
  );
}
