// Pure presentation rules for the capability map. No React, no Tauri: the
// view renders what these functions return, and `scripts/check-capability-map.ts`
// asserts them. The invariant they uphold: EVERY celebration names the
// evidence rows it stands on — no reward without a row behind it — and the
// copy never frames a failed attempt as a penalty.

import type { CapabilityEntry, SkillEvidence } from "../../lib/tauri";

export interface Celebration {
  kind: "solved" | "retained" | "applied";
  text: string;
  /** Evidence rows that justify this line (always non-empty). */
  evidenceIds: string[];
}

export interface NextStep {
  text: string;
}

const DAY_MS = 24 * 60 * 60 * 1000;

function plural(n: number, one: string, many: string): string {
  return n === 1 ? one : many;
}

/** The achievements actually earned, in the order a student earns them. */
export function celebrationsFor(entry: CapabilityEntry): Celebration[] {
  const { status } = entry;
  const out: Celebration[] = [];
  if (status.solved.achieved && status.solved.evidenceIds.length > 0) {
    out.push({
      kind: "solved",
      text: status.solvedWithAids
        ? "Superaste el reto tras reintentar o usar pistas."
        : "Resolviste el reto sin ayudas.",
      evidenceIds: status.solved.evidenceIds,
    });
  }
  if (status.retained.achieved && status.retained.evidenceIds.length > 0) {
    const days = Math.max(1, status.retainedAfterDays ?? 1);
    out.push({
      kind: "retained",
      text: `Recordaste el concepto ${days} ${plural(days, "día", "días")} después, sin ver la solución.`,
      evidenceIds: status.retained.evidenceIds,
    });
  }
  if (status.applied.achieved && status.applied.evidenceIds.length > 0) {
    out.push({
      kind: "applied",
      text: "Aplicaste el concepto en un caso nuevo, sin pistas.",
      evidenceIds: status.applied.evidenceIds,
    });
  }
  return out;
}

/** What is still open, phrased as the next step — never as a deficit. */
export function nextStepsFor(entry: CapabilityEntry, now: number): NextStep[] {
  const { status } = entry;
  const steps: NextStep[] = [];
  if (!status.solved.achieved) {
    steps.push({
      text:
        status.attemptsRecorded > 0
          ? "Sigue practicando: cada intento afina el enfoque, y no se penaliza."
          : "Aún sin practicar.",
    });
    return steps;
  }
  if (!status.retained.achieved) {
    const due = entry.nextRetrievalAtMs;
    if (due == null) {
      steps.push({ text: "El repaso se programará al cerrar la clase." });
    } else if (due <= now) {
      steps.push({ text: "Repaso disponible ahora: respóndelo de memoria." });
    } else {
      steps.push({ text: `Repaso disponible desde el ${new Date(due).toLocaleDateString()}.` });
    }
  }
  if (!status.applied.achieved) {
    steps.push({ text: "Falta aplicarlo en un caso nuevo, sin pistas." });
  }
  return steps;
}

const KIND_LABEL: Record<SkillEvidence["kind"], string> = {
  conceptual_gate: "Compuerta conceptual",
  practice_gate: "Práctica",
  retrieval: "Repaso",
  closure: "Cierre",
};

const OUTCOME_LABEL: Record<SkillEvidence["outcome"], string> = {
  passed: "Superado",
  failed: "Aún no — se siguió con más apoyo",
  escalated: "Se continuó con otro enfoque",
  self_recalled: "Dijiste que lo recordabas",
  self_forgot: "No lo recordaste",
};

export function evidenceTitle(e: SkillEvidence): string {
  const kind = e.isTransfer ? "Reto de transferencia" : KIND_LABEL[e.kind];
  return `${kind} — ${OUTCOME_LABEL[e.outcome]}`;
}

/** One-line detail: attempt, feedback rounds seen and support level. */
export function evidenceDetail(e: SkillEvidence): string {
  const parts: string[] = [`intento ${e.attemptNumber}`];
  if (e.hintsShown > 0) parts.push(`${e.hintsShown} ${plural(e.hintsShown, "ronda", "rondas")} de pistas antes`);
  if (e.supportLevel) parts.push(`apoyo: ${SUPPORT_LABEL[e.supportLevel] ?? e.supportLevel}`);
  return parts.join(" · ");
}

const SUPPORT_LABEL: Record<string, string> = {
  full: "total",
  guided: "guiado",
  faded: "atenuado",
  independent: "sin pistas",
};

/** Whole days between two timestamps, for "hace N días" style copy. */
export function daysBetween(fromMs: number, toMs: number): number {
  return Math.floor((toMs - fromMs) / DAY_MS);
}
