/**
 * Builds the model × engine capability matrix the app routes figures with
 * (`src-tauri/src/domain/visual_capabilities.json`, read by
 * `domain::visual_capabilities`) from the Rust replays of the figure evals
 * (`eval-results/*.json.rust.json`, see `eval_replay` in grounding.rs).
 *
 * An output counts AGAINST its engine only when the review rejected the
 * FIGURE (a violation about the visual, its intent or its engine); a theory
 * sent back for its word count says nothing about the engine and counts as a
 * success of the figure. Outputs that never parsed (no engine) are skipped.
 *
 *   bun scripts/build-capability-matrix.ts            # writes the JSON
 *   bun scripts/build-capability-matrix.ts --check    # prints, writes nothing
 */
import { readdirSync, readFileSync, writeFileSync } from "fs";
import { resolve } from "path";

const ROOT = resolve(import.meta.dir, "..");
const OUT = resolve(ROOT, "src-tauri/src/domain/visual_capabilities.json");
export const THRESHOLD = 0.85;
export const MIN_SAMPLES = 3;

interface Replay {
  model: string;
  scenario: string;
  verdict: { stage?: string; engine?: string | null; violations?: string[] };
}

const FIGURE_WORDS = [
  "visualaid", "visualintent", "espacial", "curvatura", "figura", "sequence_strip", "diagram_spec", "mermaid", "solid_model", "surface_3d",
  "explorable_model", "topology_flow", "interactive_module", "declarative_svg", "conceptual_matrix", "trayectoria", "geodésica",
];

/** Upper bound of the 95 % Wilson interval — same rule as `visual_capabilities::wilson_upper`. */
export function wilsonUpper(ok: number, n: number): number {
  if (n === 0) return 1;
  const z = 1.96;
  const p = ok / n;
  const denom = 1 + (z * z) / n;
  const centre = p + (z * z) / (2 * n);
  const margin = z * Math.sqrt((p * (1 - p) + (z * z) / (4 * n)) / n);
  return Math.min(1, (centre + margin) / denom);
}

/** Is this violation about the figure (and so about the engine that drew it)? */
export function aboutFigure(violation: string): boolean {
  const v = violation.toLowerCase();
  return FIGURE_WORDS.some((w) => v.includes(w));
}

export function buildMatrix(replays: Replay[]) {
  const models: Record<string, Record<string, { n: number; ok: number }>> = {};
  for (const r of replays) {
    const engine = r.verdict.engine?.split(":")[0];
    if (!engine) continue;
    const failedFigure = r.verdict.stage === "grounding" && (r.verdict.violations ?? []).some(aboutFigure);
    const cell = ((models[r.model] ??= {})[engine] ??= { n: 0, ok: 0 });
    cell.n++;
    if (!failedFigure) cell.ok++;
  }
  return { threshold: THRESHOLD, minSamples: MIN_SAMPLES, models };
}

if (import.meta.main) {
  const dir = resolve(ROOT, "eval-results");
  const files = readdirSync(dir).filter((f) => f.endsWith(".json.rust.json"));
  const replays: Replay[] = files.flatMap((f) => JSON.parse(readFileSync(resolve(dir, f), "utf-8")) as Replay[]);
  const matrix = { ...buildMatrix(replays), generatedFrom: files.length, outputs: replays.length };
  for (const [model, engines] of Object.entries(matrix.models)) {
    const cells = Object.entries(engines)
      .map(([e, c]) => `${e} ${c.ok}/${c.n}${c.n >= MIN_SAMPLES && wilsonUpper(c.ok, c.n) < THRESHOLD ? " ✗ no fiable" : ""}`)
      .join(" · ");
    console.log(`${model.padEnd(18)} ${cells}`);
  }
  if (process.argv.includes("--check")) process.exit(0);
  writeFileSync(OUT, `${JSON.stringify(matrix, null, 2)}\n`);
  console.log(`\n${replays.length} salidas de ${files.length} réplicas → ${OUT}`);
}
