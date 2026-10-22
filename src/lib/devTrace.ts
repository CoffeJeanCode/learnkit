// Developer-mode traces: the shape the backend emits as `dev://trace` plus the
// pure grouping the panel renders (no React/Tauri imports, so it is checkable
// with `bun scripts/check-dev-trace.ts`).

export interface DevTraceEntry {
  id: number;
  tsMs: number;
  /** `agent_run | tool_call | retry | critique | loop_result` */
  kind: string;
  flow?: string;
  scope?: string;
  runId?: string;
  agentId?: string;
  provider?: string;
  model?: string;
  durationMs?: number;
  systemPrompt?: string;
  input?: string;
  output?: string;
  toolCalls?: string[];
  tool?: string;
  toolArgs?: unknown;
  attempt?: number;
  verdict?: string;
  notes?: string[];
  ok?: boolean;
  errorCode?: string;
  error?: string;
  transient?: boolean;
  budgetExhausted?: boolean;
}

export interface TimelineItem {
  entry: DevTraceEntry;
  /** Tool calls that ran inside this agent run (by time window). */
  tools: DevTraceEntry[];
}

export const RING_LIMIT = 300;

/** Appends keeping the newest `RING_LIMIT`, ignoring an id already present
 *  (the initial snapshot and the live stream can overlap). */
export function appendEntry(entries: DevTraceEntry[], next: DevTraceEntry): DevTraceEntry[] {
  if (entries.some((e) => e.id === next.id)) return entries;
  const out = [...entries, next];
  return out.length > RING_LIMIT ? out.slice(out.length - RING_LIMIT) : out;
}

/** A tool call has no run id (it fires deep inside the model's tool loop), so
 *  it is attached to the agent run whose [start, end] window contains it. The
 *  run's own entry is recorded when it ENDS (`tsMs`), its start is derived from
 *  `durationMs`. Tool calls outside every window stay as standalone items. */
export function buildTimeline(entries: DevTraceEntry[]): TimelineItem[] {
  const runs = entries.filter((e) => e.kind === "agent_run");
  const owner = new Map<number, DevTraceEntry>();
  for (const t of entries) {
    if (t.kind !== "tool_call") continue;
    const run = runs.find((r) => t.tsMs >= r.tsMs - (r.durationMs ?? 0) && t.tsMs <= r.tsMs);
    if (run) owner.set(t.id, run);
  }
  const items: TimelineItem[] = [];
  for (const e of entries) {
    if (e.kind === "tool_call" && owner.has(e.id)) continue;
    const tools = e.kind === "agent_run" ? entries.filter((t) => owner.get(t.id)?.id === e.id) : [];
    items.push({ entry: e, tools });
  }
  return items;
}

export interface TraceFilter {
  onlyProblems: boolean;
  scope: string | null;
}

/** A "problem" is anything a developer debugging generation cares about: a
 *  failed run, a retry, a critic/guardrail rejection, a fail-open or backstop. */
export function isProblem(e: DevTraceEntry): boolean {
  if (e.ok === false || e.error) return true;
  if (e.kind === "retry") return true;
  const v = e.verdict ?? "";
  return /rejected|fail_open|backstop|exhausted|failed/.test(v);
}

export function filterTimeline(items: TimelineItem[], f: TraceFilter): TimelineItem[] {
  return items.filter(({ entry, tools }) => {
    if (f.scope && entry.scope && entry.scope !== f.scope) return false;
    if (!f.onlyProblems) return true;
    return isProblem(entry) || tools.some(isProblem);
  });
}
