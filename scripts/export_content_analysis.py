"""One-off export of the local LearnKit app data (SQLite course/notebook
content + the roadmap session JSON files) into readable Markdown documents,
one per course, for manual content-quality analysis.

Not part of the app itself — run manually, on demand:
    python scripts/export_content_analysis.py               # every course
    python scripts/export_content_analysis.py "producto cruz" "transporte celular"
                                                              # only courses whose
                                                              # title contains any
                                                              # of these (case-insensitive)
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
OUT_DIR = Path(__file__).resolve().parent.parent / "content-analysis"


def load_roadmap_sessions() -> dict[str, dict]:
    """Maps imported_course_id -> the roadmap session that produced it."""
    by_course = {}
    if not SESSIONS_DIR.exists():
        return by_course
    for f in SESSIONS_DIR.glob("*.json"):
        try:
            data = json.loads(f.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        course_id = data.get("imported_course_id")
        if course_id:
            by_course[course_id] = data
    return by_course


def fmt_json(value) -> str:
    return json.dumps(value, indent=2, ensure_ascii=False)


def slugify(title: str) -> str:
    ascii_ish = title.encode("ascii", "ignore").decode("ascii") or title
    slug = re.sub(r"[^a-zA-Z0-9]+", "-", ascii_ish).strip("-").lower()
    return slug or "curso"


def render_course(cur: sqlite3.Cursor, course: sqlite3.Row, session: dict | None) -> str:
    course_id = course["id"]
    lines: list[str] = []
    lines.append(f"# Curso: {course['title']}")
    lines.append("")
    lines.append(f"Generado: {datetime.now().isoformat(timespec='seconds')}")
    lines.append("")
    lines.append(f"- **id**: `{course_id}`")
    lines.append(f"- **meta**: {course['target_goal']}")
    lines.append(f"- **semanas**: {course['total_weeks']}")

    if session:
        profile = session.get("learner_profile_card") or {}
        diag = session.get("diagnostic_summary_card") or {}
        lines.append(f"- **sesión de origen**: `{session['session_id']}` ({session.get('status')})")
        if profile:
            lines.append(
                f"- **perfil del alumno**: {profile.get('entryLevel')} · "
                f"{profile.get('timeframeWeeks')} semanas · "
                f"{profile.get('weeklyCommitmentHours')} h/semana"
            )
        if diag:
            lines.append(f"- **enfoque diagnóstico**: {diag.get('coreFocus')}")
            needs = diag.get("identifiedNeeds") or []
            if needs:
                lines.append(f"- **necesidades identificadas**: {', '.join(needs)}")
            if diag.get("learningStrategy"):
                lines.append(f"- **estrategia de aprendizaje**: {diag['learningStrategy']}")
    lines.append("")

    milestones = cur.execute(
        "SELECT * FROM syllabus_milestones WHERE course_id = ? ORDER BY week_number", (course_id,)
    ).fetchall()

    lines.append("## Hitos y clases")
    lines.append("")
    lines.append("| Semana | Hito | Entregable | Clase | ¿Notebook generado? |")
    lines.append("|---|---|---|---|---|")
    for m in milestones:
        classes = cur.execute(
            "SELECT * FROM classes WHERE milestone_id = ? ORDER BY order_index", (m["id"],)
        ).fetchall()
        for c in classes:
            doc = cur.execute("SELECT * FROM notebook_documents WHERE class_id = ?", (c["id"],)).fetchone()
            lines.append(
                f"| {m['week_number']} | {m['title']} | {m['deliverable_goal']} | "
                f"{c['title']} | {'sí (' + doc['status'] + ')' if doc else 'no'} |"
            )
    lines.append("")

    # Full generated content, block by block, for every class that has one.
    for m in milestones:
        classes = cur.execute(
            "SELECT * FROM classes WHERE milestone_id = ? ORDER BY order_index", (m["id"],)
        ).fetchall()
        for c in classes:
            doc = cur.execute("SELECT * FROM notebook_documents WHERE class_id = ?", (c["id"],)).fetchone()
            if not doc:
                continue
            blocks = cur.execute(
                "SELECT * FROM notebook_blocks WHERE document_id = ? ORDER BY order_index", (doc["id"],)
            ).fetchall()

            lines.append(f"## Notebook: {c['title']}")
            lines.append("")
            lines.append(
                f"- **class_id**: `{c['id']}`  ·  **document_id**: `{doc['id']}`  ·  **status**: {doc['status']}"
            )
            lines.append(f"- **bloques**: {len(blocks)}")
            lines.append("")
            for b in blocks:
                content = json.loads(b["content_json"])
                title = content.get("title")
                heading = f"### {b['order_index'] + 1}. `{b['block_type']}`"
                if title:
                    heading += f" — {title}"
                lines.append(heading)
                lines.append("")
                lines.append("```json")
                lines.append(fmt_json(content))
                lines.append("```")
                lines.append("")

    return "\n".join(lines)


def main() -> None:
    filters = [f.lower() for f in sys.argv[1:]]

    con = sqlite3.connect(f"file:{DB_PATH}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    cur = con.cursor()

    courses = cur.execute("SELECT * FROM courses ORDER BY id").fetchall()
    if filters:
        courses = [c for c in courses if any(f in c["title"].lower() for f in filters)]
        if not courses:
            print("Ningún curso coincide con esos filtros.")
            return

    sessions_by_course = load_roadmap_sessions()
    OUT_DIR.mkdir(exist_ok=True)

    for course in courses:
        content = render_course(cur, course, sessions_by_course.get(course["id"]))
        out_path = OUT_DIR / f"{slugify(course['title'])}.md"
        out_path.write_text(content, encoding="utf-8")
        print(f"Escrito: {out_path} ({out_path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
