"""Export de las sesiones de roadmap a documentos Markdown para análisis.

Cada sesión genera un documento con:
  - la conversación completa con el usuario (messages),
  - el roadmap (perfil del alumno, diagnóstico y sílabo con módulos),
  - las clases del curso importado (si existe) con el estado de sus notebooks.

No forma parte de la app — se ejecuta a mano:
    python scripts/export_sessions.py                    # todas las sesiones
    python scripts/export_sessions.py "ux" "derivadas"   # solo sesiones cuyo
                                                         # título contenga algo
    python scripts/export_sessions.py --full             # además, el JSON
                                                         # completo de cada bloque
"""

import json
import re
import sqlite3
import sys
from datetime import datetime
from pathlib import Path

APPDATA = Path.home() / "AppData" / "Local" / "com.learnkit.app" / "learnkit"
DB_PATH = APPDATA / "notebook.sqlite3"
SESSIONS_DIR = APPDATA / "roadmap_sessions"
OUT_DIR = Path(__file__).resolve().parent.parent / "session-analysis"

ROLE_LABELS = {"user": "Usuario", "assistant": "Asistente"}
LEVEL_LABELS = {
    "absolute_zero": "Cero absoluto",
    "theoretical_foundations": "Fundamentos teóricos",
    "applied_intermediate": "Nivel medio que busca aplicar",
}
STATUS_LABELS = {"sealed": "sellada", "active": "activa"}
PHASE_LABELS = {"onboarding": "onboarding", "diagnostic": "diagnóstico", "roadmap": "roadmap"}


def fmt_ts(ms: int | None) -> str:
    if not ms:
        return "—"
    return datetime.fromtimestamp(ms / 1000).strftime("%Y-%m-%d %H:%M:%S")


def slugify(title: str) -> str:
    ascii_ish = title.encode("ascii", "ignore").decode("ascii") or title
    slug = re.sub(r"[^a-zA-Z0-9]+", "-", ascii_ish).strip("-").lower()
    return slug or "sesion"


def session_title(session: dict) -> str:
    custom = (session.get("custom_title") or "").strip()
    if custom:
        return custom
    profile = session.get("learner_profile_card") or {}
    if (profile.get("topic") or "").strip():
        return profile["topic"].strip()
    draft_topic = (session.get("draft") or {}).get("topic")
    if isinstance(draft_topic, str) and draft_topic.strip():
        return draft_topic.strip()
    return "Sesión sin título"


def load_sessions(filters: list[str]) -> list[dict]:
    sessions: list[dict] = []
    if not SESSIONS_DIR.exists():
        return sessions
    for f in SESSIONS_DIR.glob("*.json"):
        try:
            data = json.loads(f.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        if not isinstance(data, dict) or "session_id" not in data:
            continue
        if filters:
            title = session_title(data).lower()
            if not any(flt in title for flt in filters):
                continue
        sessions.append(data)
    sessions.sort(key=lambda s: s.get("updated_at_ms") or 0, reverse=True)
    return sessions


def render_messages(session: dict) -> list[str]:
    lines = ["## Conversación con el usuario", ""]
    messages = session.get("messages") or []
    if not messages:
        lines += ["_Sin mensajes guardados en esta sesión._", ""]
        return lines
    for i, msg in enumerate(messages, start=1):
        role = ROLE_LABELS.get(msg.get("role", ""), msg.get("role", "?"))
        lines.append(f"**{i}. {role}** · {fmt_ts(msg.get('at_ms'))}")
        lines.append("")
        lines.append((msg.get("text") or "").strip() or "_vacío_")
        lines.append("")
    return lines


def render_profile(profile: dict, heading: str = "### Perfil del alumno") -> list[str]:
    level = LEVEL_LABELS.get(profile.get("entryLevel", ""), profile.get("entryLevel", "—"))
    return [
        heading,
        "",
        f"- **tema**: {profile.get('topic', '—')}",
        f"- **objetivo**: {profile.get('targetGoal', '—')}",
        f"- **nivel de entrada**: {level}",
        f"- **plazo**: {profile.get('timeframeWeeks', '—')} semanas",
        f"- **compromiso semanal**: {profile.get('weeklyCommitmentHours', '—')} h/semana",
        f"- **horas totales disponibles**: {profile.get('totalAvailableHours', '—')} h",
        "",
    ]


def render_diagnostic(diag: dict, heading: str = "### Resumen del diagnóstico") -> list[str]:
    lines = [heading, ""]
    if diag.get("coreFocus"):
        lines.append(f"- **enfoque central**: {diag['coreFocus']}")
    needs = diag.get("identifiedNeeds") or []
    if needs:
        lines.append("- **necesidades identificadas**:")
        lines += [f"  - {n}" for n in needs]
    if diag.get("learningStrategy"):
        lines.append(f"- **estrategia de aprendizaje**: {diag['learningStrategy']}")
    lines.append("")
    return lines


def render_syllabus(syllabus: dict) -> list[str]:
    lines = ["### Sílabo", ""]
    lines.append(f"- **título del curso**: {syllabus.get('courseTitle', '—')}")
    lines.append(f"- **semanas**: {syllabus.get('totalWeeks', '—')}")
    lines.append(f"- **ritmo**: {syllabus.get('paceHoursPerWeek', '—')} h/semana")
    lines.append("")
    for m in syllabus.get("milestones") or []:
        week = m.get("week", "?")
        raw_title = str(m.get("title", "")).strip()
        # Algunos títulos ya vienen como "Semana 1 (3 días): ..." — no duplicar.
        heading = raw_title if re.match(rf"^semana\s*{week}\b", raw_title, flags=re.I) else f"Semana {week} — {raw_title}"
        lines.append(f"#### {heading}")
        lines.append("")
        if m.get("deliverable"):
            lines.append(f"- **entregable**: {m['deliverable']}")
        modules = m.get("micromodules") or []
        if modules:
            lines.append("- **micromódulos**:")
            for mod in modules:
                hours = mod.get("hours")
                head = f"  - **{mod.get('label', '')}**"
                if hours is not None:
                    head += f" ({hours} h)"
                head += f" — {mod.get('deliverable', '')}"
                lines.append(head)
                blocks = mod.get("interactiveBlocks") or []
                if blocks:
                    lines.append(f"    - bloques interactivos: {', '.join(blocks)}")
        lines.append("")
    return lines


def render_roadmap(session: dict) -> list[str]:
    lines = ["## Roadmap", ""]
    profile = session.get("learner_profile_card")
    diag = session.get("diagnostic_summary_card")
    sealed = session.get("roadmap_package")
    proposed = session.get("proposed_plan")

    if not (profile or diag or sealed or proposed):
        lines += ["_Aún no hay roadmap en esta sesión._", ""]
        return lines

    if sealed:
        lines += [f"_Roadmap sellado_ · generado {fmt_ts(sealed.get('generated_at_ms'))}", ""]
        profile = profile or sealed.get("learner_profile")
        diag = diag or sealed.get("diagnostic_summary")
        syllabus = sealed.get("syllabus") or {}
        battery = sealed.get("diagnostic_battery")
    elif proposed:
        lines += [
            "_Plan propuesto, pendiente de confirmación por el alumno (no sellado)._",
            "",
            f"- **pregunta de cierre**: {proposed.get('closingQuestion', '—')}",
            "",
        ]
        diag = diag or proposed.get("diagnosticSummary")
        syllabus = proposed.get("syllabus") or {}
        battery = None
    else:
        syllabus = {}
        battery = None

    if profile:
        lines += render_profile(profile)
    if diag:
        lines += render_diagnostic(diag)
    if syllabus:
        lines += render_syllabus(syllabus)
    if battery:
        lines += render_battery(battery)
    return lines


def render_battery(battery: dict) -> list[str]:
    """Batería diagnóstica congelada en el roadmap (snapshot al sellar)."""
    lines = ["### Batería diagnóstica (snapshot)", ""]
    if battery.get("goalAlignment"):
        lines.append(f"- **alineación con el objetivo**: {battery['goalAlignment']}")
    questions = battery.get("questions") or []
    lines.append(f"- **preguntas**: {len(questions)}")
    lines.append("")
    for i, q in enumerate(questions, start=1):
        lines.append(f"{i}. ({q.get('dimension', '?')}) {q.get('prompt', '')}")
        for opt in q.get("options") or []:
            lines.append(f"   - {opt}")
        if q.get("diagnosticInsight"):
            lines.append(f"   - _lectura del error_: {q['diagnosticInsight']}")
    lines.append("")
    return lines


def render_pending_battery(session: dict) -> list[str]:
    """Batería en curso sobre la sesión (aún no importada a un curso)."""
    battery = session.get("pending_diagnostic_battery")
    if not battery:
        return []
    lines = ["## Batería diagnóstica (en curso)", ""]
    if battery.get("goalAlignment"):
        lines.append(f"- **alineación con el objetivo**: {battery['goalAlignment']}")
    answers = battery.get("answers") or {}
    questions = battery.get("questions") or []
    lines.append(f"- **respondidas**: {len(answers)}/{len(questions)}")
    lines.append("")
    for i, q in enumerate(questions):
        lines.append(f"{i + 1}. ({q.get('dimension', '?')}) {q.get('prompt', '')}")
        chosen = answers.get(str(i))
        for opt in q.get("options") or []:
            mark = " ✅" if opt == chosen else ""
            lines.append(f"   - {opt}{mark}")
        lines.append("")
    return lines


def render_classes(con: sqlite3.Connection, session: dict, full: bool) -> list[str]:
    course_id = session.get("imported_course_id")
    if not course_id:
        return []
    lines = ["## Clases", ""]
    course = con.execute("SELECT * FROM courses WHERE id = ?", (course_id,)).fetchone()
    if not course:
        lines += [f"_Curso `{course_id}` no encontrado en la base local (¿borrado?)._", ""]
        return lines

    lines += [
        f"- **curso**: {course['title']} (`{course_id}`)",
        f"- **meta**: {course['target_goal']}",
        f"- **semanas**: {course['total_weeks']}",
        f"- **primera clase**: `{session.get('first_class_id') or '—'}`",
        "",
        "### Hitos y clases",
        "",
        "| Semana | Hito | Entregable | Clase | Notebook |",
        "|---|---|---|---|---|",
    ]
    milestones = con.execute(
        "SELECT * FROM syllabus_milestones WHERE course_id = ? ORDER BY week_number",
        (course_id,),
    ).fetchall()
    classes: list[sqlite3.Row] = []
    for m in milestones:
        rows = con.execute(
            "SELECT * FROM classes WHERE milestone_id = ? ORDER BY order_index", (m["id"],)
        ).fetchall()
        for c in rows:
            classes.append(c)
            doc = con.execute(
                "SELECT * FROM notebook_documents WHERE class_id = ?", (c["id"],)
            ).fetchone()
            state = f"sí ({doc['status']})" if doc else "no"
            lines.append(
                f"| {m['week_number']} | {m['title']} | {m['deliverable_goal']} | "
                f"{c['title']} | {state} |"
            )
    lines.append("")

    for c in classes:
        doc = con.execute(
            "SELECT * FROM notebook_documents WHERE class_id = ?", (c["id"],)
        ).fetchone()
        lines.append(f"#### {c['title']}")
        lines.append("")
        if not doc:
            lines += ["_Notebook no generado todavía._", ""]
            continue
        blocks = con.execute(
            "SELECT * FROM notebook_blocks WHERE document_id = ? ORDER BY order_index",
            (doc["id"],),
        ).fetchall()
        lines.append(
            f"- **class_id**: `{c['id']}` · **document_id**: `{doc['id']}` · "
            f"**estado**: {doc['status']} · **bloques**: {len(blocks)}"
        )
        lines.append("")
        for b in blocks:
            content = json.loads(b["content_json"])
            title = content.get("title")
            heading = f"- `{b['block_type']}`"
            if title:
                heading += f" — {title}"
            lines.append(heading)
            if full:
                lines.append("")
                lines.append("  ```json")
                lines.extend(
                    "  " + row for row in json.dumps(content, indent=2, ensure_ascii=False).splitlines()
                )
                lines.append("  ```")
        lines.append("")
    return lines


def render_session(con: sqlite3.Connection | None, session: dict, full: bool) -> str:
    lines = [f"# Sesión: {session_title(session)}", ""]
    lines += [
        f"- **session_id**: `{session.get('session_id')}`",
        f"- **estado**: {STATUS_LABELS.get(session.get('status', ''), session.get('status'))}"
        f" · **fase**: {PHASE_LABELS.get(session.get('phase', ''), session.get('phase'))}",
        f"- **creada**: {fmt_ts(session.get('created_at_ms'))}"
        f" · **actualizada**: {fmt_ts(session.get('updated_at_ms'))}",
        f"- **turnos en fase**: {session.get('turn_count_in_phase', 0)}"
        f" · **fallos de grounding consecutivos**: {session.get('consecutive_grounding_failures', 0)}",
        f"- **curso importado**: `{session.get('imported_course_id') or '—'}`",
        "",
    ]
    rejections = session.get("last_rejection_reasons") or []
    if rejections:
        lines += ["**Últimos rechazos de grounding:**", ""]
        lines += [f"- {r}" for r in rejections]
        lines.append("")

    lines += render_messages(session)
    lines += render_roadmap(session)
    lines += render_pending_battery(session)
    if con is not None:
        lines += render_classes(con, session, full)
    return "\n".join(lines).rstrip() + "\n"


def session_filename(session: dict, used: set[str]) -> str:
    short_id = (session.get("session_id") or "")[:8]
    base = f"{slugify(session_title(session))}-{short_id}"
    name, n = base, 2
    while name in used:
        name = f"{base}-{n}"
        n += 1
    used.add(name)
    return f"{name}.md"


def write_index(all_sessions: list[dict], filenames: dict[str, str]) -> Path:
    """Índice con TODAS las sesiones exportadas en disco (no solo las de esta corrida)."""
    lines = [
        "# Sesiones exportadas",
        "",
        f"Exportado: {datetime.now().isoformat(timespec='seconds')}",
        "",
        "| Título | Estado | Fase | Mensajes | Curso importado | Archivo |",
        "|---|---|---|---|---|---|",
    ]
    for s in all_sessions:
        fname = filenames[s["session_id"]]
        if not (OUT_DIR / fname).exists():
            continue
        lines.append(
            f"| {session_title(s)} | {STATUS_LABELS.get(s.get('status', ''), s.get('status'))} "
            f"| {PHASE_LABELS.get(s.get('phase', ''), s.get('phase'))} "
            f"| {len(s.get('messages') or [])} "
            f"| `{s.get('imported_course_id') or '—'}` | [{fname}]({fname}) |"
        )
    path = OUT_DIR / "INDICE.md"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def main() -> None:
    args = [a for a in sys.argv[1:] if a != "--full"]
    full = "--full" in sys.argv
    filters = [a.lower() for a in args]

    sessions = load_sessions(filters)
    if not sessions:
        print("Ninguna sesión coincide con esos filtros (o no hay sesiones).")
        return

    all_sessions = load_sessions([])
    # Nombres calculados sobre la lista completa para que una corrida con
    # filtros no cambie los archivos ni el índice de las demás.
    filenames: dict[str, str] = {}
    used: set[str] = set()
    for s in all_sessions:
        filenames[s["session_id"]] = session_filename(s, used)

    con = None
    if DB_PATH.exists():
        con = sqlite3.connect(f"file:{DB_PATH}?mode=ro", uri=True)
        con.row_factory = sqlite3.Row
    else:
        print(f"Aviso: no hay base de datos en {DB_PATH}; se omite el detalle de clases.")

    OUT_DIR.mkdir(exist_ok=True)

    for session in sessions:
        out_path = OUT_DIR / filenames[session["session_id"]]
        content = render_session(con, session, full)
        out_path.write_text(content, encoding="utf-8")
        print(f"Escrito: {out_path} ({out_path.stat().st_size} bytes)")

    index_path = write_index(all_sessions, filenames)
    print(f"Índice: {index_path}")
    if con:
        con.close()


if __name__ == "__main__":
    main()
