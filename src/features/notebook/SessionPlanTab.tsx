import { ClassPath } from "../../components/ClassPath";
import { MicromoduleItem } from "../../components/MicromoduleItem";
import { ENTRY_LEVEL_LABEL } from "../../lib/schemas";
import type { ClassRecord, RoadmapSession } from "../../lib/schemas";

// Same user-facing phase labels as RoadmapView's tracker — this is the
// same journey, seen from inside the session page.
const PHASES: { id: string; label: string }[] = [
  { id: "onboarding", label: "1. Tu meta" },
  { id: "diagnostic", label: "2. Diagnóstico" },
  { id: "roadmap", label: "3. Tu plan" },
];

/** The notebook's "Plan" tab: the sealed roadmap that produced this course
 *  — how you got here (meta → diagnóstico → plan, the cards that record
 *  each step) plus how you're going (the class path with locks and checks).
 *  Owns no data: everything rides on the session resolved by the notebook
 *  view and the classes already in the nav store. */
export function SessionPlanTab({
  session,
  loading,
  classes,
  activeClassId,
  onSelectClass,
  onOpenConversation,
  error,
}: {
  session: RoadmapSession | null;
  loading: boolean;
  classes: ClassRecord[];
  activeClassId: string | null;
  onSelectClass: (classId: string) => void;
  onOpenConversation: () => void;
  /** Load failure for the session's classes (surfaced by the container view
   *  — this component owns no data, so it can't retry on its own). */
  error?: string | null;
}) {
  const activeClass = activeClassId ? classes.find((c) => c.id === activeClassId) : undefined;
  const resumeButton = activeClassId ? (
    <button className="btn-primary" onClick={() => onSelectClass(activeClassId)}>
      Retomar “{activeClass?.title ?? "la clase"}”
    </button>
  ) : null;
  const errorBanner = error ? <div className="alert error">{error}</div> : null;

  if (loading) {
    return (
      <div className="plan-tab">
        <p className="muted">Cargando tu plan…</p>
      </div>
    );
  }

  const pkg = session?.roadmap_package ?? null;
  if (!pkg) {
    return (
      <div className="plan-tab">
        <div className="plan-tab-head">
          <div>
            <h2>Tu plan</h2>
            <p className="muted">El plan de esta sesión todavía no está disponible.</p>
          </div>
          <div className="row">
            <button className="btn-quiet" onClick={onOpenConversation}>
              ← Ver conversación
            </button>
            {resumeButton}
          </div>
        </div>
        {errorBanner}
        {classes.length > 0 && (
          <div className="card class-path-card">
            <div className="card-head">
              <h3>Tu ruta — cómo vas</h3>
            </div>
            <ClassPath classes={classes} activeClassId={activeClassId} onSelect={onSelectClass} />
          </div>
        )}
      </div>
    );
  }

  const profileCard = session?.learner_profile_card ?? null;
  const diagnosticCard = session?.diagnostic_summary_card ?? null;
  const phaseIndex = session ? PHASES.findIndex((p) => p.id === session.phase) : -1;
  const sealed = session?.status === "sealed";

  return (
    <div className="plan-tab">
      <div className="plan-tab-head">
        <div>
          <h2>Tu plan</h2>
          <p className="muted">Cómo llegó tu ruta hasta aquí — y cómo vas.</p>
        </div>
        <div className="row">
          <button className="btn-quiet" onClick={onOpenConversation}>
            ← Ver conversación
          </button>
          {resumeButton}
        </div>
      </div>

      {errorBanner}

      {/* How you got here: the three steps of the journey, each one backed
          by the card it produced. */}
      <div className="phase-tracker">
        {PHASES.map((p, i) => (
          <div
            key={p.id}
            className={
              "phase-step" +
              (i === phaseIndex ? " active" : "") +
              (i < phaseIndex || sealed ? " done" : "")
            }
          >
            {p.label}
          </div>
        ))}
      </div>

      {profileCard && (
        <div className="card learner-profile-card">
          <div className="card-head">
            <h3>{profileCard.topic}</h3>
            <span className="badge ok">meta confirmada</span>
          </div>
          <p className="hint">{profileCard.targetGoal}</p>
          <p className="hint">
            {profileCard.timeframeWeeks} semanas · {profileCard.weeklyCommitmentHours} h/semana (
            {profileCard.totalAvailableHours} h en total) ·{" "}
            {ENTRY_LEVEL_LABEL[profileCard.entryLevel] ?? profileCard.entryLevel}
          </p>
        </div>
      )}

      {diagnosticCard && (
        <div className="card diagnostic-card">
          <div className="card-head">
            <h3>Cómo vamos a abordarlo</h3>
            <span className="badge ok">diagnóstico listo</span>
          </div>
          <p className="hint">{diagnosticCard.coreFocus}</p>
          <ul className="dod-list">
            {diagnosticCard.identifiedNeeds.map((need) => (
              <li key={need}>{need}</li>
            ))}
          </ul>
          <p>{diagnosticCard.learningStrategy}</p>
        </div>
      )}

      <div className="card final-package">
        <div className="card-head">
          <h3>{pkg.syllabus.courseTitle}</h3>
          <span className="badge ok">guardado</span>
        </div>
        <p className="hint">
          {pkg.syllabus.totalWeeks} semanas · {pkg.syllabus.paceHoursPerWeek} h/semana
        </p>
        <ul className="dod-list">
          {pkg.syllabus.milestones.map((m) => (
            <li key={m.week} className="done">
              <strong>
                Semana {m.week}: {m.title}
              </strong>{" "}
              — {m.deliverable}
              <ul className="dod-list">
                {m.micromodules.map((mod, i) => (
                  <MicromoduleItem key={i} mod={mod} className="done" />
                ))}
              </ul>
            </li>
          ))}
        </ul>

        {classes.length > 0 && (
          <>
            <h4 className="side-note-title">Tu ruta — cómo vas</h4>
            <ClassPath classes={classes} activeClassId={activeClassId} onSelect={onSelectClass} />
          </>
        )}
      </div>
    </div>
  );
}
