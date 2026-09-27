import { create } from "zustand";
import type { ChatTurn, RoadmapSession, RoadmapSessionSummary } from "../lib/schemas";
import {
  answerDiagnosticQuestion,
  deleteRoadmapSession,
  getRoadmapSession,
  listRoadmapSessions,
  renameRoadmapSession,
  retryRoadmapTurn,
  sendRoadmapMessage,
  skipDiagnosticBattery,
  startRoadmapSession,
  tauriError,
} from "../lib/tauri";
import type { ChatMessage } from "../types";

const HISTORY_FILTER_KEY = "learnkit-restore-history";

/** Error codes worth retrying: the model never produced output (network
 *  blip, unreachable host, timeout), so the same input can just run again —
 *  the mirror of `AppError::is_transient` on the Rust side. Anything else
 *  (missing key, bad input, persistence) needs a fix, not a retry. */
const TRANSIENT_ERROR_CODES = new Set(["AgentExecutionFailed", "Timeout", "ProviderConnectionFailed"]);

/** One extra automatic attempt AT THE UI LEVEL, on top of the backend's own
 *  `MAX_EXECUTION_ATTEMPTS` budget — a brief outage shouldn't require the
 *  student to notice the error and click anything for it to heal. Capped at 1
 *  so a dead provider surfaces with the manual button instead of looping
 *  forever behind a spinner. */
const MAX_AUTO_RETRIES = 1;

/** Pause between the surfaced failure and the automatic re-attempt: long
 *  enough that a connection refused a moment ago isn't refused again
 *  instantly, short enough that "Reintentando…" doesn't read as a hang. */
const AUTO_RETRY_DELAY_MS = 900;

type TurnPayload = { message: string; session: RoadmapSession };

/** Persisted filter: whether reopening a session restores its saved chat.
 *  On by default (the whole point of the log); localStorage keeps the
 *  user's choice across restarts. */
function readHistoryFilter(): boolean {
  try {
    return window.localStorage.getItem(HISTORY_FILTER_KEY) !== "0";
  } catch {
    return true;
  }
}

/** Backend `ChatTurn` -> UI `ChatMessage` (same role vocabulary). */
function toChatMessages(turns: ChatTurn[]): ChatMessage[] {
  return turns
    .filter((t) => t.role === "user" || t.role === "assistant")
    .map((t) => ({ role: t.role as ChatMessage["role"], content: t.text }));
}

/** Drops the trailing `⚠ nota` bubbles so a retried turn replaces the
 *  failure instead of stacking a new one under it (older errors further up
 *  the conversation stay as history). */
function dropTrailingErrors(messages: ChatMessage[]): ChatMessage[] {
  let end = messages.length;
  while (end > 0 && messages[end - 1].role === "error") end -= 1;
  return messages.slice(0, end);
}

interface RoadmapState {
  session: RoadmapSession | null;
  messages: ChatMessage[];
  sending: boolean;
  error: string | null;
  /** The last turn failed on a RETRYABLE error (network/timeout): the alert
   *  offers the manual "Reintentar" button next to the message. */
  retryable: boolean;
  /** Non-null while a retry runs: `"auto"` = the unsolicited re-attempt after
   *  a transient failure, `"manual"` = the student clicked. Drives the
   *  "Reintentando…" status so a retry never looks like a fresh think. */
  retryPhase: "auto" | "manual" | null;
  /** Auto retries still allowed for the CURRENT turn — reset to
   *  `MAX_AUTO_RETRIES` on every student-initiated action and drained by
   *  `failTurn`, so each action gets at most one automatic second wave. */
  autoRetriesLeft: number;
  /** The battery answer whose turn failed — kept so a retry can re-submit it
   *  (the answer only lands on disk together with the turn that consumed it). */
  pendingAnswer: { questionIndex: number; answer: string } | null;
  /** Bumped by reset/openSession so late responses from a superseded turn are
   *  discarded instead of resurrecting stale state. */
  epoch: number;
  /** Saved sessions for the drawer (summaries, newest first). */
  saved: RoadmapSessionSummary[];
  savedLoading: boolean;
  /** True while a saved session is being opened. */
  opening: boolean;
  /** First interaction creates AND uses the session in one go —
   *  no "Iniciar sesión" gate in the UI. */
  start: () => Promise<void>;
  send: (input: string) => Promise<void>;
  /** Sends the message, starting a session first if there isn't one yet. */
  sendOrStart: (input: string) => Promise<void>;
  /** Re-runs the turn that just failed WITHOUT duplicating it: the battery
   *  answer if that's what failed, otherwise `retry_roadmap_turn` (which
   *  resumes from the already-persisted user turn). Backs the alert's manual
   *  "Reintentar" button. */
  retry: () => Promise<void>;
  /** Records one answer to the pending diagnostic battery. The last answer
   *  triggers the Roadmap-stage turn automatically — its reply (if any)
   *  lands in `messages` like a normal turn. */
  answerDiagnostic: (questionIndex: number, answer: string) => Promise<void>;
  /** Escape hatch: finishes the Diagnostic stage with whatever's answered
   *  so far. */
  skipBattery: () => Promise<void>;
  refreshSession: () => Promise<void>;
  /** Clears the draft and bumps the epoch (discards any in-flight turn).
   *  Never touches the DB — a session only exists once a message sends. */
  reset: () => void;
  refreshSaved: () => Promise<void>;
  /** Opens a saved session, restoring its persisted conversation (user +
   *  assistant turns — see `ChatTurn` in Rust). The `restoreHistory` filter
   *  mutes the chat restore without touching what's on disk. */
  openSession: (id: string) => Promise<void>;
  renameSession: (id: string, title: string) => Promise<void>;
  deleteSession: (id: string) => Promise<void>;
  /** Replaces the open session in place (e.g. after `ensureCourseImported`
   *  backfills ids on a session sealed before the SQLite import existed). */
  setSession: (session: RoadmapSession) => void;
  /** Filter toggle: restore saved chat on reopen (persisted). */
  restoreHistory: boolean;
  setRestoreHistory: (on: boolean) => void;
}

export const useRoadmap = create<RoadmapState>((set, get) => {
  /** Applies a successful turn: session swap + assistant bubble (skipped when
   *  the turn legitimately produced no text, e.g. a mid-battery answer). */
  function applySuccess(epoch: number, result: TurnPayload) {
    if (get().epoch !== epoch) return;
    set((s) => ({
      sending: false,
      retryPhase: null,
      retryable: false,
      pendingAnswer: null,
      session: result.session,
      ...(result.message ? { messages: [...s.messages, { role: "assistant" as const, content: result.message }] } : {}),
    }));
  }

  /** Shared failure path for every turn (send, battery answer, retry):
   *  a transient error gets ONE automatic re-attempt after a short pause —
   *  still inside `sending`, so the status row switches to "Reintentando…"
   *  instead of flashing an error the student would have to dismiss. If that
   *  fails too (or the error isn't transient), surface the message + bubble
   *  and set `retryable` so the alert can offer the manual button. */
  async function failTurn(e: unknown, epoch: number, retryFn: () => Promise<TurnPayload>): Promise<void> {
    const err = tauriError(e);
    const retryable = TRANSIENT_ERROR_CODES.has(err.code);
    if (retryable && get().epoch === epoch && get().autoRetriesLeft > 0) {
      set((s) => ({ autoRetriesLeft: s.autoRetriesLeft - 1, retryPhase: "auto", error: null }));
      await new Promise((resolve) => setTimeout(resolve, AUTO_RETRY_DELAY_MS));
      if (get().epoch !== epoch) return;
      try {
        applySuccess(epoch, await retryFn());
      } catch (nested) {
        // The budget is drained now, so recursion lands on the surface branch.
        await failTurn(nested, epoch, retryFn);
      }
      return;
    }
    if (get().epoch !== epoch) return;
    const text = `${err.code}: ${err.message}`;
    set((s) => ({
      sending: false,
      retryPhase: null,
      retryable,
      error: text,
      messages: [...s.messages, { role: "error", content: text }],
    }));
  }

  /** One turn end-to-end. `attempt` is the action the student took;
   *  `retryFn` is what the automatic re-attempt (and the manual button, via
   *  `retry`) replays — it must NOT re-record the user's input, since a
   *  failed attempt has already persisted it. */
  async function runTurn(epoch: number, attempt: () => Promise<TurnPayload>, retryFn: () => Promise<TurnPayload>) {
    try {
      applySuccess(epoch, await attempt());
    } catch (e) {
      await failTurn(e, epoch, retryFn);
    }
  }

  /** Shared entry state for a student-initiated turn: clears stale errors,
   *  re-arms the auto-retry budget, and drops the failure bubbles the retry
   *  is about to replace. */
  function beginTurn(extra: Partial<RoadmapState> = {}): number {
    const epoch = get().epoch;
    set((s) => ({
      sending: true,
      error: null,
      retryable: false,
      retryPhase: null,
      autoRetriesLeft: MAX_AUTO_RETRIES,
      messages: dropTrailingErrors(s.messages),
      ...extra,
    }));
    return epoch;
  }

  return {
    session: null,
    messages: [],
    sending: false,
    error: null,
    retryable: false,
    retryPhase: null,
    autoRetriesLeft: 0,
    pendingAnswer: null,
    epoch: 0,
    saved: [],
    savedLoading: false,
    opening: false,
    restoreHistory: readHistoryFilter(),

    start: async () => {
      // Guard against double-invocation (React StrictMode mounts effects
      // twice; the auto-start effect can also re-fire when provider state
      // settles). A second `start_roadmap_session` would launch a second
      // long-lived LLM call and orphan the first session.
      if (get().sending) return;
      const epoch = beginTurn({ messages: [], session: null, pendingAnswer: null });
      try {
        const { message, session } = await startRoadmapSession();
        if (get().epoch !== epoch) return;
        set({ session, messages: [{ role: "assistant", content: message }], sending: false, retryPhase: null });
      } catch (e) {
        if (get().epoch !== epoch) return;
        const err = tauriError(e);
        // The greeting is local (no model call): a failure here is
        // persistence/config, so there is nothing a retry could replay.
        set({ sending: false, retryPhase: null, retryable: false, error: `${err.code}: ${err.message}` });
      }
    },

    send: async (input) => {
      const { session, sending } = get();
      if (sending || !session || !input.trim() || session.status === "sealed") return;
      const epoch = beginTurn({ pendingAnswer: null, messages: [...get().messages, { role: "user", content: input }] });
      const sessionId = session.session_id;
      await runTurn(
        epoch,
        () => sendRoadmapMessage(sessionId, input),
        // The failed send already persisted this user turn (see Rust
        // `advance`), so the re-attempt resumes from it instead of
        // appending it to the log a second time.
        () => retryRoadmapTurn(sessionId),
      );
    },

    refreshSession: async () => {
      const { session } = get();
      if (!session) return;
      try {
        const fresh = await getRoadmapSession(session.session_id);
        set({ session: fresh });
      } catch (e) {
        set({ error: tauriError(e).message });
      }
    },

    reset: () =>
      set((s) => ({
        session: null,
        messages: [],
        sending: false,
        error: null,
        retryable: false,
        retryPhase: null,
        autoRetriesLeft: 0,
        pendingAnswer: null,
        epoch: s.epoch + 1,
      })),

    sendOrStart: async (input) => {
      if (!input.trim() || get().sending || get().opening) return;
      if (!get().session) {
        await get().start();
        // Start failed (missing key, network…) — stop here, error is shown.
        if (!get().session) return;
      }
      await get().send(input);
    },

    retry: async () => {
      const { session, sending, pendingAnswer } = get();
      if (sending || !session || !get().retryable) return;
      const answer = pendingAnswer;
      const epoch = beginTurn({ retryPhase: "manual" });
      const sessionId = session.session_id;
      const attempt = () =>
        answer ? answerDiagnosticQuestion(sessionId, answer.questionIndex, answer.answer) : retryRoadmapTurn(sessionId);
      // Same call for the student's click and for a failed click's own
      // auto-wave: `retry_roadmap_turn`/re-answer never duplicate input.
      await runTurn(epoch, attempt, attempt);
    },

    answerDiagnostic: async (questionIndex, answer) => {
      const { session, sending } = get();
      if (sending || !session) return;
      const epoch = beginTurn({ pendingAnswer: { questionIndex, answer } });
      const sessionId = session.session_id;
      const submit = () => answerDiagnosticQuestion(sessionId, questionIndex, answer);
      await runTurn(epoch, submit, submit);
    },

    skipBattery: async () => {
      const { session, sending } = get();
      if (sending || !session) return;
      const epoch = beginTurn();
      const sessionId = session.session_id;
      const submit = () => skipDiagnosticBattery(sessionId);
      await runTurn(epoch, submit, submit);
    },

    refreshSaved: async () => {
      set({ savedLoading: true });
      try {
        const saved = await listRoadmapSessions();
        set({ saved, savedLoading: false });
      } catch (e) {
        set({ savedLoading: false });
        // Non-critical: the drawer just shows an empty state. The error
        // surfaces wherever the user is actually working.
      }
    },

    openSession: async (id) => {
      if (get().sending || get().opening) return;
      const epoch = get().epoch + 1;
      set({
        session: null,
        messages: [],
        sending: false,
        error: null,
        retryable: false,
        retryPhase: null,
        autoRetriesLeft: 0,
        pendingAnswer: null,
        opening: true,
        epoch,
      });
      try {
        const session = await getRoadmapSession(id);
        if (get().epoch !== epoch) return;
        // The persisted log restores the conversation; the filter (a UI
        // choice, persisted) mutes the restore without touching the disk copy.
        set({ session, messages: get().restoreHistory ? toChatMessages(session.messages) : [], opening: false });
      } catch (e) {
        if (get().epoch !== epoch) return;
        set({ error: tauriError(e).message, opening: false });
      }
    },

    setRestoreHistory: (on) => {
      set({ restoreHistory: on });
      try {
        window.localStorage.setItem(HISTORY_FILTER_KEY, on ? "1" : "0");
      } catch {
        // localStorage unavailable (webview restrictions) — in-memory only.
      }
      // Re-applying to the OPEN session: off collapses it to the recovered
      // hint, on brings the saved chat back without a round-trip.
      const session = get().session;
      if (session) {
        set({ messages: on ? toChatMessages(session.messages) : [] });
      }
    },

    renameSession: async (id, title) => {
      try {
        const updated = await renameRoadmapSession(id, title);
        set((s) => ({ saved: s.saved.map((x) => (x.session_id === id ? updated : x)) }));
      } catch (e) {
        set({ error: tauriError(e).message, retryable: false });
      }
    },

    setSession: (session) => {
      const current = get().session;
      if (current?.session_id !== session.session_id) return;
      set({ session });
    },

    deleteSession: async (id) => {
      try {
        await deleteRoadmapSession(id);
        // Deleting the open conversation drops back to the idle prompt.
        set((s) => ({
          saved: s.saved.filter((x) => x.session_id !== id),
          ...(s.session?.session_id === id
            ? {
                session: null,
                messages: [],
                sending: false,
                error: null,
                retryable: false,
                retryPhase: null,
                autoRetriesLeft: 0,
                pendingAnswer: null,
                epoch: s.epoch + 1,
              }
            : {}),
        }));
      } catch (e) {
        set({ error: tauriError(e).message, retryable: false });
      }
    },
  };
});
