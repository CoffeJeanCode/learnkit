// "Where I left it", per session: which section the student was in, which class,
// and the last block they had reached. Pure model (no React, no stores) so the
// landing rules and the persistence can be checked on their own. Kept on this
// device; if it is missing or stale the session simply opens on its usual screen.

export type ResumeView = "roadmap" | "plan" | "notebook";

export interface Resume {
  view: ResumeView;
  /** The class being read when `view` is "notebook". */
  classId?: string;
  /** The last block the student reached, and the class it belongs to (a block id is only useful there). */
  blockId?: string;
  blockClassId?: string;
}

export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const resumeKey = (sessionId: string) => `learnkit.resume.${sessionId}`;

const VIEWS: readonly string[] = ["roadmap", "plan", "notebook"];

function isResume(v: unknown): v is Resume {
  if (typeof v !== "object" || v === null) return false;
  const r = v as Record<string, unknown>;
  const text = (x: unknown) => x === undefined || (typeof x === "string" && x.length > 0);
  return typeof r.view === "string" && VIEWS.includes(r.view) && text(r.classId) && text(r.blockId) && text(r.blockClassId);
}

/** The saved position of a session; anything missing, corrupt or from a store that throws gives null. */
export function loadResume(store: KeyValueStore | null, sessionId: string): Resume | null {
  try {
    const raw = store?.getItem(resumeKey(sessionId));
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    return isResume(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

/** Merges `patch` into what is saved. Best effort: a store that throws is ignored. */
export function saveResume(store: KeyValueStore | null, sessionId: string, patch: Partial<Resume>): void {
  try {
    const current = loadResume(store, sessionId) ?? { view: "roadmap" as ResumeView };
    const next: Resume = { ...current, ...patch };
    // Leaving the notebook keeps the class and block (so coming back resumes), but a view
    // other than the notebook never carries a stale class on its own.
    store?.setItem(resumeKey(sessionId), JSON.stringify(next));
  } catch {
    /* private window, blocked site data, quota: ignore */
  }
}

export interface SessionLike {
  status: string;
  imported_course_id?: string | null;
}

export interface Landing {
  view: ResumeView;
  courseId?: string;
  classId?: string;
  blockId?: string;
}

/** Where opening a session should land. A session still being built opens on its
 *  conversation; a sealed one resumes the section it was left in, and inside the
 *  notebook the exact class and block. With nothing saved a sealed session opens
 *  on its plan (the content that is waiting). */
export function landing(session: SessionLike | null, resume: Resume | null): Landing {
  if (!session || session.status !== "sealed") return { view: "roadmap" };
  const courseId = session.imported_course_id ?? undefined;
  if (resume?.view === "notebook" && courseId && resume.classId) {
    const sameClass = resume.blockClassId === resume.classId;
    return { view: "notebook", courseId, classId: resume.classId, blockId: sameClass ? resume.blockId : undefined };
  }
  if (resume?.view === "roadmap") return { view: "roadmap" };
  return { view: "plan" };
}

export function browserStore(): KeyValueStore | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}
