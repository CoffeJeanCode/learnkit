// Real per-block generation cost, read from the dev-trace entries the backend
// already records (`block_generation_timing`, see `log_block_timing` in
// `notebook_service/generation.rs`). This is the BASELINE for any speed work:
// unlike `scripts/eval-block-models.ts --dry-run`, nothing here is simulated.
// Pure (no React/Tauri imports) so `bun scripts/check-block-timing.ts` checks it.

import type { DevTraceEntry } from "./devTrace";

export const BLOCK_TIMING_VERDICT = "block_generation_timing";

export interface BlockTiming {
  position: number;
  blockType: string;
  source: "model" | "backstop" | string;
  attempts: number;
  elapsedMs: number;
  criticDeferred: boolean;
}

function noteMap(notes: string[] | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  for (const n of notes ?? []) {
    const i = n.indexOf("=");
    if (i > 0) out[n.slice(0, i)] = n.slice(i + 1);
  }
  return out;
}

/** The timing a trace entry carries, or `null` when it is not a timing entry
 *  (or is malformed — a bad row never poisons the summary). */
export function parseBlockTiming(e: DevTraceEntry): BlockTiming | null {
  if (e.verdict !== BLOCK_TIMING_VERDICT) return null;
  const m = noteMap(e.notes);
  const elapsedMs = Number(m.elapsed_ms);
  const attempts = Number(m.attempts);
  if (!m.block_type || !Number.isFinite(elapsedMs) || !Number.isFinite(attempts)) return null;
  return {
    position: Number(m.position) || 0,
    blockType: m.block_type,
    source: m.source ?? "model",
    attempts,
    elapsedMs,
    criticDeferred: m.critic_deferred === "true",
  };
}

export interface TimingRow {
  blockType: string;
  count: number;
  meanMs: number;
  p50Ms: number;
  p90Ms: number;
  meanAttempts: number;
  /** Share of blocks that needed more than one generator attempt. */
  retryRate: number;
  /** Share shipped by the backstop (every attempt was rejected). */
  backstopRate: number;
}

/** Nearest-rank percentile of an ascending-sorted list. */
export function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length - 1, Math.max(0, rank - 1))];
}

const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);

function row(blockType: string, items: BlockTiming[]): TimingRow {
  const ms = items.map((t) => t.elapsedMs).sort((a, b) => a - b);
  return {
    blockType,
    count: items.length,
    meanMs: Math.round(mean(ms)),
    p50Ms: percentile(ms, 50),
    p90Ms: percentile(ms, 90),
    meanAttempts: Number(mean(items.map((t) => t.attempts)).toFixed(2)),
    retryRate: Number((items.filter((t) => t.attempts > 1).length / items.length).toFixed(3)),
    backstopRate: Number((items.filter((t) => t.source === "backstop").length / items.length).toFixed(3)),
  };
}

/** One row per block type (slowest first) plus an `ALL` row. */
export function summarizeBlockTiming(entries: DevTraceEntry[]): TimingRow[] {
  const timings = entries.map(parseBlockTiming).filter((t): t is BlockTiming => t !== null);
  if (timings.length === 0) return [];
  const byType = new Map<string, BlockTiming[]>();
  for (const t of timings) byType.set(t.blockType, [...(byType.get(t.blockType) ?? []), t]);
  const rows = [...byType.entries()].map(([type, items]) => row(type, items)).sort((a, b) => b.meanMs - a.meanMs);
  return [...rows, row("ALL", timings)];
}

/** Markdown table for a report / PR description. */
export function timingTable(rows: TimingRow[]): string {
  if (rows.length === 0) return "(sin entradas `block_generation_timing`)";
  const head = "| Bloque | n | media | p50 | p90 | intentos | con reintento | backstop |\n|---|---|---|---|---|---|---|---|";
  const pct = (x: number) => `${Math.round(x * 100)}%`;
  const body = rows.map((r) => `| ${r.blockType} | ${r.count} | ${r.meanMs} ms | ${r.p50Ms} ms | ${r.p90Ms} ms | ${r.meanAttempts} | ${pct(r.retryRate)} | ${pct(r.backstopRate)} |`);
  return [head, ...body].join("\n");
}
