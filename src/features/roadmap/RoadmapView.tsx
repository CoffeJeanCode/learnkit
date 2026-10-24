import { useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ThinkingSketch, ToolSketch, WaitingHint } from "../../components/AgentLoader";
import { ClassPath } from "../../components/ClassPath";
import { DiagnosticBatteryCard } from "../../components/DiagnosticBatteryCard";
import { DiagnosticBatteryStage } from "../../components/DiagnosticBatteryStage";
import { MicromoduleItem } from "../../components/MicromoduleItem";
import { ENTRY_LEVEL_LABEL } from "../../lib/schemas";
import type { ClassRecord, RoadmapPhase } from "../../lib/schemas";
import { ensureCourseImported, listCourseClasses, onAgentEvent, onPlanPart, tauriError } from "../../lib/tauri";
import { useNotebookNav } from "../../stores/notebook";
import { useProviders } from "../../stores/providers";
import { useRoadmap } from "../../stores/roadmap";
import { useUi } from "../../stores/ui";
import { PlanAdjustPanel } from "./PlanAdjustPanel";
import { PlanDraftView } from "./PlanDraftView";
import { ProfileForm } from "./ProfileForm";

// User-facing labels for the phase tracker — deliberately about the
// milestone the student reaches, not the agent's internal vocabulary.
// "2. Diagnóstico" and "3. Tu plan" now close together in the same turn as
// "1. Tu meta" (see `roadmap_agent`'s autonomy rules): there is no
// confirmation step in between anymore.
const PHASES: { id: RoadmapPhase; label: string }[] = [
  { id: "onboarding", label: "1. Tu meta" },
  { id: "diagnostic", label: "2. Diagnóstico" },
  { id: "roadmap", label: "3. Tu plan" },
];

// Roles rendered as handwritten margin annotations, not system labels.
const ROLE_LABEL: Record<string, string> = {
  assistant: "mentor ✎",
  user: "tú →",
  error: "⚠ nota",
};

// Tool id → what the student reads while the agent runs it (agent://tool).
const TOOL_LABEL: Record<string, string> = {
  submit_diagnostic_assessment: "Guardando tu diagnóstico…",
  present_diagnostic_battery: "Diseñando tu batería…",
  propose_syllabus_plan: "Armando tu plan…",
  submit_plan_outline: "Esbozando tus semanas…",
  submit_week_sessions: "Detallando las sesiones de cada semana…",
  propose_capstone_project: "Pensando tu proyecto final…",
  confirm_syllabus_plan: "Escribiendo tu primera clase…",
  publish_class_notebook: "Escribiendo el notebook…",
};

// Step 0 of onboarding: connect a provider before the diagnostic conversation
// can start. Shown only while no provider has a key configured — env-var
// auto-detection (see backend `seed_keys_from_env`) usually skips this
// entirely for a developer who already has e.g. `ANTHROPIC_API_KEY` set.
function ProviderSetup() {
  const { providers, busyId, error, saveKey } = useProviders();
  const [providerId, setProviderId] = useState("");
  const [apiKey, setApiKey] = useState("");
  const busy = busyId === providerId;

  // Saving the key updates the shared providers store; RoadmapView reacts to
  // that (`anyConfigured` flips true) and offers the "Iniciar sesión"
  // prompt — the conversation itself starts only on explicit user action,
  // never here (and `saveKey` swallows its own errors, so starting here
  // would even fire after a failed save).
  const submit = async () => {
    if (!providerId || !apiKey.trim()) return;
    await saveKey(providerId, apiKey);
    setApiKey("");
  };

  return (
    <div className="view roadmap">
      <h2>Bienvenido a LearnKit</h2>
      <p className="muted">
        Antes de empezar tu diagnóstico de aprendizaje, conecta un proveedor de IA. Tu API key se
        guarda cifrada en este equipo (Stronghold) y nunca se comparte ni se vuelve a mostrar.
      </p>

      <label>
        Proveedor
        <select value={providerId} onChange={(e) => setProviderId(e.target.value)}>
          <option value="">— elige uno —</option>
          {providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>

      <label>
        API key
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          placeholder="sk-…"
          autoComplete="off"
        />
      </label>

      {error && <div className="alert error">{error}</div>}

      <div className="row">
        <button
          className="btn-primary"
          disabled={busy || !providerId || !apiKey.trim()}
          onClick={submit}
        >
          {busy ? "Guardando…" : "Guardar y comenzar"}
        </button>
      </div>
    </div>
  );
}

export function RoadmapView() {
  const {
    session,
    messages,
    sending,
    opening,
    error,
    retryable,
    retryPhase,
    sendOrStart,
    proposeQuestions,
    submitProfile,
    confirmPlan,
    adjustPlan,
    resumeHandOff,
    retry,
    setSession,
    answerDiagnostic,
    restoreHistory,
    setRestoreHistory,
  } = useRoadmap();
  const { providers, loading: providersLoading, refresh: refreshProviders } = useProviders();
  const planDraft = useRoadmap((s) => s.planDraft);
  const receivePlanPart = useRoadmap((s) => s.receivePlanPart);
  const [input, setInput] = useState("");
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const openNotebooks = useNotebookNav((s) => s.open);
  const setView = useUi((s) => s.setView);
  const [notebookError, setNotebookError] = useState<string | null>(null);
  const [openingNotebook, setOpeningNotebook] = useState(false);
  const [planClasses, setPlanClasses] = useState<ClassRecord[]>([]);

  useEffect(() => {
    refreshProviders();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A fresh session started from the header leaves stale composer text
  // behind — clear it whenever the conversation identity changes.
  const sessionId = session?.session_id;
  useEffect(() => {
    setInput("");
  }, [sessionId]);

  const courseId = session?.imported_course_id;
  const firstClassId = session?.first_class_id;

  // Landing on the seal itself: when THIS turn is what sealed the session,
  // the plan just became real, so the student is taken to the Plan section —
  // the header owns that section now (`SessionPlanView`), which is also what
  // makes this jump safe: it fires on the one-shot `justSealed` transition
  // flag, never on a re-mount. The old code keyed on `status === "sealed"`,
  // so coming BACK here (e.g. "← Ver conversación" from Plan) re-fired it and
  // bounced the student straight out of the screen they'd just asked for.
  const justSealed = useRoadmap((s) => s.justSealed);
  const consumeJustSealed = useRoadmap((s) => s.consumeJustSealed);
  useEffect(() => {
    if (!justSealed || !session) return;
    if (!consumeJustSealed()) return;
    let cancelled = false;
    const land = () => {
      if (!cancelled) setView("plan");
    };
    // Guaranteed at seal time for anything sealed through the gates; the
    // recovery only matters for sessions sealed before the import existed.
    if (courseId) {
      land();
      return;
    }
    ensureCourseImported(session.session_id)
      .then((recovered) => {
        if (cancelled) return;
        setSession(recovered);
        land();
      })
      .catch((e) => {
        if (!cancelled) setNotebookError(tauriError(e).message);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [justSealed, session?.session_id, courseId]);

  // Fallback only: if the auto-navigate above fails (surfaced via
  // `notebookError`), this still lists the classes so the "Ver tu primera
  // clase" button and the path below can recover manually instead of
  // leaving the student stuck on an empty plan screen.
  useEffect(() => {
    if (session?.status !== "sealed") {
      setPlanClasses([]);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        let cId = courseId;
        if (!cId) {
          const recovered = await ensureCourseImported(session.session_id);
          if (cancelled) return;
          setSession(recovered);
          cId = recovered.imported_course_id ?? undefined;
        }
        if (!cId) return;
        const classes = await listCourseClasses(cId);
        if (!cancelled) setPlanClasses(classes);
      } catch {
        // Non-critical for this list — the auto-navigate/manual-button
        // paths above already surface a `notebookError` for real failures.
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session?.status, session?.session_id, courseId]);

  const anyConfigured = providers.some((p) => p.configured);

  // No auto-start: visiting this view must never create a session on its
  // own (that filled the drawer with empty sessions). The conversation
  // begins only on explicit interaction — the "Iniciar sesión" button below
  // or "Nueva sesión" in the header.

  const submit = () => {
    if (!input.trim()) return;
    sendOrStart(input);
    setInput("");
  };

  // Auto-scroll: new turns (user echo, mentor reply, or the "Pensando…"
  // placeholder appearing) should always bring the latest line into view.
  // Exception: while the diagnostic battery is on screen there is no scroll
  // at all — the student is reading/answering a fixed card and jumping to
  // the bottom yanks the questions out from under them (hold position).
  const messagesEndRef = useRef<HTMLDivElement>(null);
  // Scroll the real scroller (`.workspace`) to its ABSOLUTE bottom instead
  // of aligning a sentinel with `scrollIntoView({ block: "end" })`: the
  // composer is a sticky dock floating over the bottom ~60px of the
  // viewport, so any alignment that stops short of max scroll leaves the
  // "Pensando…" row (and the tail of the last message) hidden underneath
  // the floating input.
  const scrollChatToEnd = () => {
    const end = messagesEndRef.current;
    if (!end) return;
    const scroller = end.closest(".workspace");
    if (scroller instanceof HTMLElement) {
      scroller.scrollTo({ top: scroller.scrollHeight, behavior: "smooth" });
    } else {
      end.scrollIntoView({ behavior: "smooth", block: "end" });
    }
  };
  // Truthy only while the student still has unanswered questions — once the
  // last one is answered (or the model proposes a plan straight away for
  // `absolute_zero`), the battery stays on the session but is DONE: it must
  // no longer hold the composer hostage or re-render as an open card, since
  // by then a `proposed_plan` (or the sealed package) is what's current.
  const pb = session?.pending_diagnostic_battery ?? null;
  const diagnosticOpen = !!pb && Object.keys(pb.answers).length < pb.questions.length;
  useEffect(() => {
    if (diagnosticOpen) return;
    scrollChatToEnd();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages.length, sending, opening, diagnosticOpen, error]);
  // The battery now lives INSIDE `.messages` (it's the mentor's next turn, not
  // a card parked below the chat), so it needs one scroll of its own: the
  // effect above deliberately holds position while it's open, which would
  // otherwise leave the freshly mounted questions below the fold. Only on the
  // closed→open transition — every later message is the other effect's job.
  const wasDiagOpen = useRef(diagnosticOpen);
  useEffect(() => {
    if (wasDiagOpen.current !== diagnosticOpen) {
      wasDiagOpen.current = diagnosticOpen;
      if (diagnosticOpen) scrollChatToEnd();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [diagnosticOpen]);

  // Real activity, not a timer: while a turn runs, every tool call the
  // model makes emits `agent://tool` — so the status row can show the gear
  // figure (tools grinding) instead of the thinking spiral. The run's
  // `agent://completed` (and the turn resolving) drop it back.
  const [workingTool, setWorkingTool] = useState<string | null>(null);
  useEffect(() => {
    if (!sending) setWorkingTool(null);
  }, [sending]);
  useEffect(() => {
    let alive = true;
    let unlisteners: UnlistenFn[] = [];
    (async () => {
      const acquired = await Promise.all([
        onAgentEvent("agent://tool", (e) => {
          if (alive && e.tool) setWorkingTool(e.tool);
        }),
        onAgentEvent("agent://completed", () => {
          if (alive) setWorkingTool(null);
        }),
      ]);
      if (alive) {
        unlisteners = acquired;
      } else {
        acquired.forEach((u) => u());
      }
    })();
    return () => {
      alive = false;
      unlisteners.forEach((u) => u());
    };
  }, []);

  // The plan arrives in parts while the proposal is written (outline, then each
  // week): the store folds them into `planDraft`, rendered below with a fade-in.
  useEffect(() => {
    let alive = true;
    let unlisten: UnlistenFn | null = null;
    onPlanPart(receivePlanPart).then((u) => {
      if (alive) unlisten = u;
      else u();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [receivePlanPart]);

  if (providersLoading) {
    return (
      <div className="view roadmap">
        <p className="muted">Cargando…</p>
      </div>
    );
  }

  if (!anyConfigured) {
    return <ProviderSetup />;
  }

  const phaseIndex = session ? PHASES.findIndex((p) => p.id === session.phase) : -1;
  const sealed = session?.status === "sealed";
  const pkg = session?.roadmap_package ?? null;
  const profileCard = session?.learner_profile_card ?? null;
  const diagnosticCard = session?.diagnostic_summary_card ?? null;
  const pendingBattery = session?.pending_diagnostic_battery ?? null;
  const proposedPlan = session?.proposed_plan ?? null;
  // The form is Gate 1 for a session that has no profile AND no saved student
  // turn yet (server truth: `session.messages`, not the optimistic bubble, so
  // the form stays mounted — busy — while the first submit is in flight).
  // A chat-era session already mid-conversation keeps its composer instead.
  const hasSavedUserTurn = session?.messages.some((m) => m.role === "user") ?? false;
  const showForm = !profileCard && (!session || !hasSavedUserTurn);
  // The profile is saved but nothing was produced behind it (battery, or the
  // proposal for `absolute_zero`) and there is no transient error to retry on:
  // offer the step explicitly instead of leaving the student at a dead end.
  const handOffStalled = !!profileCard && !sealed && !pendingBattery && !proposedPlan && !sending && !opening && !error;

  // Opens `targetClassId` (any class in the path), defaulting to the first
  // class when called from the plain "Ver tu primera clase" button.
  const openClassManually = async (targetClassId?: string) => {
    if (!session) return;
    setOpeningNotebook(true);
    setNotebookError(null);
    try {
      let cId = courseId;
      let clsId = targetClassId ?? firstClassId;
      if (!cId || !clsId) {
        const recovered = await ensureCourseImported(session.session_id);
        setSession(recovered);
        cId = recovered.imported_course_id ?? undefined;
        clsId = targetClassId ?? recovered.first_class_id ?? undefined;
      }
      if (!cId || !clsId) {
        setNotebookError("No se pudo generar tu primera clase todavía. Intenta de nuevo.");
        return;
      }
      const classes = await listCourseClasses(cId);
      openNotebooks(classes, clsId, cId);
      setView("notebook");
    } catch (e) {
      setNotebookError(tauriError(e).message);
    } finally {
      setOpeningNotebook(false);
    }
  };

  return (
    <div className="view roadmap">
      <div className={showForm ? "empty-hero" : undefined}>
        <h2>Tu ruta de aprendizaje</h2>
        <p className="muted">
          {showForm
            ? "Cuéntame qué quieres aprender y cuánto tiempo tienes. Te haré unas preguntas sobre tu tema y armo tu plan y tu primera clase."
            : "Tu plan se arma con lo que me contaste. Revísalo, ajústalo si hace falta y confírmalo."}
        </p>
        {showForm && (
          <ProfileForm
            initialBasics={session?.profile_basics ?? null}
            questions={session?.pending_context_questions ?? null}
            busy={sending || opening}
            serverError={error}
            onPropose={(basics) => void proposeQuestions(basics)}
            onSubmit={(answers) => void submitProfile(answers)}
          />
        )}
      </div>

      {session && (
        <>
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
                {profileCard.totalAvailableHours} h en total) · {ENTRY_LEVEL_LABEL[profileCard.entryLevel] ?? profileCard.entryLevel}
              </p>
            </div>
          )}
        </>
      )}

      {/* La conversación va antes de las tarjetas de estado: el diagnóstico
          y la propuesta son el RESULTADO del último turno, así que deben
          aparecer después del último mensaje, nunca antes — mantiene la
          secuencia cronológica que el estudiante ya está leyendo. */}
      <div className="messages">
        {session && messages.length > 0 && (
          // Filter for THIS conversation's messages: on (default) the saved
          // chat shows and keeps growing; off collapses it to the hint below
          // — the log on disk is intact either way.
          <label className="check history-filter" title="Muestra u oculta los mensajes guardados de esta conversación">
            <input type="checkbox" checked={restoreHistory} onChange={(e) => setRestoreHistory(e.target.checked)} />
            Mensajes guardados ✎ {restoreHistory ? "(visibles)" : "(ocultos)"}
          </label>
        )}
        {messages.map((m, i) => (
          <div key={i} className={`msg ${m.role}`}>
            <div className="msg-role">{ROLE_LABEL[m.role] ?? m.role}</div>
            <div className="msg-body">{m.content}</div>
          </div>
        ))}
        {session && messages.length === 0 && !sending && !opening && (
          <p className="muted">
            ✎ Sesión recuperada del diario
            {restoreHistory ? "" : " (mensajes ocultos por el filtro)"}. ¡Sigue donde quedaste!
          </p>
        )}

        {/* El diagnóstico y la batería SON parte de la conversación, no un
            apéndice: van DENTRO de `.messages`, en su orden cronológico, para
            que las preguntas se lean como el siguiente turno del mentor y no
            como un formulario suelto colgado debajo del chat. */}
        {session && (
          <>
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

            {diagnosticOpen && pendingBattery && (
              <DiagnosticBatteryStage
                battery={pendingBattery}
                onAnswer={(index, answer) => answerDiagnostic(index, answer)}
                busy={sending}
              />
            )}
          </>
        )}
      </div>

      {/* La propuesta de plan es un estado RESUELTO del último turno, así que
          va justo después del chat, y el indicador de "pensando" (para el
          turno SIGUIENTE, todavía en curso) va debajo de ellos, no encima. */}
      {session && (
        <>
          {sending && planDraft && !sealed && <PlanDraftView draft={planDraft} />}

          {/* While a revised plan is being written, the draft stands in for the
              old proposal (two plans on screen would be confusing); if the
              revision fails the old one comes back untouched. */}
          {proposedPlan && !sealed && !(sending && planDraft) && (
            <div className="card final-package proposed-plan-card">
              <div className="card-head">
                <h3>{proposedPlan.syllabus.courseTitle}</h3>
                <span className="badge">propuesta — sin confirmar</span>
              </div>
              <p className="hint">
                {proposedPlan.syllabus.totalWeeks} semanas · {proposedPlan.syllabus.paceHoursPerWeek} h/semana
              </p>
              <ul className="dod-list milestone-list">
                {proposedPlan.syllabus.milestones.map((m) => (
                  <li key={m.week}>
                    <strong>
                      Semana {m.week}: {m.title}
                    </strong>{" "}
                    — {m.deliverable}
                    {m.weeklyGoal && (
                      <blockquote className="week-goal">
                        <strong>Meta semanal:</strong> {m.weeklyGoal}
                      </blockquote>
                    )}
                    <ul className="dod-list">
                      {m.micromodules.map((mod, i) => (
                        <MicromoduleItem key={i} mod={mod} />
                      ))}
                    </ul>
                  </li>
                ))}
              </ul>
              <p>{proposedPlan.closingQuestion}</p>
              <div className="row">
                <button className="btn-primary" onClick={() => void confirmPlan()} disabled={sending}>
                  Confirmar
                </button>
                {sending && <span className="hint">Un momento…</span>}
              </div>
              <PlanAdjustPanel
                key={`${proposedPlan.syllabus.courseTitle}-${proposedPlan.syllabus.totalWeeks}-${proposedPlan.syllabus.paceHoursPerWeek}`}
                current={{ weeks: proposedPlan.syllabus.totalWeeks, hours: proposedPlan.syllabus.paceHoursPerWeek }}
                busy={sending}
                onAdjust={(adjustment) => void adjustPlan(adjustment)}
              />
            </div>
          )}

          {pkg && (
            <div className="card final-package">
              <div className="card-head">
                <h3>{pkg.syllabus.courseTitle}</h3>
                <span className="badge ok">guardado</span>
              </div>
              <p className="hint">
                {pkg.syllabus.totalWeeks} semanas · {pkg.syllabus.paceHoursPerWeek} h/semana
              </p>
              <ul className="dod-list milestone-list">
                {pkg.syllabus.milestones.map((m) => (
                  <li key={m.week} className="done">
                    <strong>
                      Semana {m.week}: {m.title}
                    </strong>{" "}
                    — {m.deliverable}
                    {m.weeklyGoal && (
                      <blockquote className="week-goal">
                        <strong>Meta semanal:</strong> {m.weeklyGoal}
                      </blockquote>
                    )}
                    <ul className="dod-list">
                      {m.micromodules.map((mod, i) => (
                        <MicromoduleItem key={i} mod={mod} className="done" />
                      ))}
                    </ul>
                  </li>
                ))}
              </ul>

              {courseId && <DiagnosticBatteryCard courseId={courseId} />}

              <div className="row">
                <button className="btn-primary" onClick={() => openClassManually()} disabled={openingNotebook}>
                  {openingNotebook ? "Abriendo…" : "Ver tu primera clase"}
                </button>
                {notebookError && <span className="hint">{notebookError}</span>}
              </div>

              {planClasses.length > 0 && (
                <>
                  <h4 className="side-note-title">Tus clases</h4>
                  <ClassPath classes={planClasses} activeClassId={null} onSelect={(id) => openClassManually(id)} />
                </>
              )}
            </div>
          )}
        </>
      )}

      {opening && (
        <div className="msg status">
          <div className="msg-body">Abriendo sesión…</div>
        </div>
      )}
      {sending && (
        <div className="msg status">
          <div className="msg-body">
            {workingTool ? (
              <ToolSketch label={TOOL_LABEL[workingTool] ?? "Trabajando…"} />
            ) : (
              // "Reintentando…" during the automatic second wave (and the
              // manual one) — a retry must not read as a fresh "Pensando…",
              // or it looks like the same call is just hanging again.
              <ThinkingSketch label={retryPhase ? "Reintentando…" : "Pensando…"} />
            )}
            {/* Keyed by the active label so the stopwatch restarts when the
                phase changes (thinking -> tool, or into a retry). */}
            <WaitingHint key={`${workingTool ?? "think"}-${retryPhase ? "retry" : "first"}`} />
          </div>
        </div>
      )}

      {handOffStalled && (
        <div className="alert error">
          <span className="alert-text">No pude armar el siguiente paso todavía. Tu perfil ya está guardado.</span>
          <button className="btn-quiet btn-retry" onClick={() => void resumeHandOff()}>
            Reintentar
          </button>
        </div>
      )}

      {error && !showForm && (
        <div className="alert error">
          <span className="alert-text">{error}</span>
          {retryable && (
            <button className="btn-quiet btn-retry" onClick={() => void retry()} disabled={sending}>
              Reintentar
            </button>
          )}
        </div>
      )}

      {session && !sealed && !diagnosticOpen && !profileCard && !showForm && (
        // Solo para sesiones de la era del chat que ya estaban a medias (con
        // mensajes pero sin perfil): las nuevas usan el formulario de arriba.
        // Dock siempre visible durante la conversación. Oculto SOLO mientras
        // quedan preguntas sin responder (`diagnosticOpen`): esa espera se
        // resuelve con clics (DiagnosticBatteryStage). Una vez respondida la
        // batería — incluida toda la etapa de propuesta/ajuste del plan — el
        // composer vuelve a estar visible, porque confirmar o pedir cambios
        // es de nuevo texto libre. El primer envío (sin sesión) usa el
        // composer propio del `empty-hero` de arriba, no este.
        <div className="row composer dock">
          <textarea
            ref={composerRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              // Enter sends; Shift+Enter keeps its default (a newline) so
              // longer answers to the mentor can span multiple lines.
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
            placeholder="Responde al mentor…"
            rows={3}
            disabled={sending}
            autoFocus
          />
          <button className="btn-primary" onClick={submit} disabled={sending || !input.trim()}>
            Enviar
          </button>
        </div>
      )}

      {/* Sentinel MUST stay the LAST node of the view: it anchors the
          auto-scroll effect above (via `closest(".workspace")`) and its
          fallback `scrollIntoView` — placing it before the sticky dock made
          that fallback stop short of the bottom, where the floating composer
          covers the "Pensando…" row and the tail of the last message. */}
      <div ref={messagesEndRef} />
    </div>
  );
}
