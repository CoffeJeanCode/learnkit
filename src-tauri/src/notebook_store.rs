//! SQLite persistence for the Notebook engine (courses -> syllabus_milestones
//! -> classes -> notebook_documents -> notebook_blocks). Separate from
//! `persistence::FileStore` (JSON config/session files) by design: this is
//! genuinely relational data with real foreign keys, which a directory of
//! JSON blobs can't express cleanly.
//!
//! Like `FileStore`, methods here are synchronous and called directly from
//! async command/service code without `spawn_blocking` — local SQLite
//! reads/writes on the tiny per-class payloads this app deals with are well
//! under a millisecond, so the same pragmatic tradeoff `FileStore` already
//! makes applies here too.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::domain::learner_memory::LearnerCognitiveMemory;
use crate::domain::notebook::{
    BlockStatus, BlockUpdate, ClassRecord, Course, DiagnosticBattery, DiagnosticBatteryState, DynamicBlockType,
    NotebookBlock, NotebookDocument, NotebookPayload, NotebookStatus, SyllabusMilestone,
};
use crate::error::{AppError, AppResult};

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS courses (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    target_goal TEXT NOT NULL,
    total_weeks INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    diagnostic_battery_json TEXT
);

CREATE TABLE IF NOT EXISTS syllabus_milestones (
    id TEXT PRIMARY KEY,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    week_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    deliverable_goal TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_milestones_course ON syllabus_milestones(course_id);

CREATE TABLE IF NOT EXISTS classes (
    id TEXT PRIMARY KEY,
    milestone_id TEXT NOT NULL REFERENCES syllabus_milestones(id) ON DELETE CASCADE,
    class_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    hours REAL NOT NULL DEFAULT 0,
    objective TEXT
);
CREATE INDEX IF NOT EXISTS idx_classes_milestone ON classes(milestone_id);

CREATE TABLE IF NOT EXISTS notebook_documents (
    id TEXT PRIMARY KEY,
    class_id TEXT NOT NULL UNIQUE REFERENCES classes(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    pedagogical_rationale TEXT,
    current_block_index INTEGER NOT NULL DEFAULT 0,
    initial_prediction TEXT
);

CREATE TABLE IF NOT EXISTS notebook_blocks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES notebook_documents(id) ON DELETE CASCADE,
    block_type TEXT NOT NULL,
    content_json TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'ready',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_feedback TEXT
);
CREATE INDEX IF NOT EXISTS idx_blocks_document ON notebook_blocks(document_id);

-- One row per learner (today, always `learner_memory::LOCAL_LEARNER_ID` —
-- see that module's doc comment) — the WHOLE `LearnerCognitiveMemory`
-- serialized as JSON, same "don't invent a relational shape for a
-- backend-only blob" precedent as `courses.diagnostic_battery_json`.
CREATE TABLE IF NOT EXISTS learner_memory (
    learner_id TEXT PRIMARY KEY,
    data_json TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
"#;

/// One-time migration: an sqlite file written before the fixed-4-section
/// notebook model was retired has `notebook_blocks.section_type TEXT NOT
/// NULL` — every insert since (which no longer supplies it) fails with a
/// `NOT NULL constraint failed` `PersistenceError`. Safe to just drop and
/// recreate both tables: their content is entirely agent-generated and
/// regenerates on demand (`NotebookService::generate_class_notebook`);
/// `courses`/`syllabus_milestones`/`classes` — the real, non-regenerable
/// bookkeeping — are untouched.
fn migrate_retired_section_type_column(conn: &Connection) -> AppResult<()> {
    let table_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'notebook_blocks'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !table_exists {
        return Ok(()); // fresh database — nothing to migrate
    }

    let mut stmt = conn.prepare("PRAGMA table_info(notebook_blocks)")?;
    let has_section_type = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == "section_type");
    drop(stmt);

    if has_section_type {
        tracing::warn!(
            "migrating notebook_blocks/notebook_documents off the retired fixed-section schema \
             (dropping and recreating — content regenerates on demand)"
        );
        conn.execute_batch("DROP TABLE IF EXISTS notebook_blocks; DROP TABLE IF EXISTS notebook_documents;")?;
    }
    Ok(())
}

/// One-time migration: a `courses` table created before the diagnostic
/// battery moved from a notebook block to a course-level field is missing
/// `diagnostic_battery_json` — `CREATE TABLE IF NOT EXISTS` is a no-op
/// against it, so add the column directly if it's not already there.
/// Nullable, so existing rows are simply `NULL` (no battery on record).
fn migrate_add_diagnostic_battery_column(conn: &Connection) -> AppResult<()> {
    let table_exists: bool = conn
        .query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'courses'", [], |_| Ok(()))
        .optional()?
        .is_some();
    if !table_exists {
        return Ok(()); // fresh database — the CREATE TABLE above already has the column
    }

    let mut stmt = conn.prepare("PRAGMA table_info(courses)")?;
    let has_column = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == "diagnostic_battery_json");
    drop(stmt);

    if !has_column {
        conn.execute("ALTER TABLE courses ADD COLUMN diagnostic_battery_json TEXT", [])?;
    }
    Ok(())
}

/// One-time migration: `classes` created before one-class-per-micromodule
/// (it used to always be one class per week) is missing `hours`, and
/// `notebook_documents` created before the free-text rationale field
/// replaced the closed `pedagogicalProfile` enum is missing
/// `pedagogical_rationale` — same additive `ALTER TABLE` pattern as
/// `migrate_add_diagnostic_battery_column`: nullable/defaulted, existing
/// rows are simply backfilled with `0`/`NULL`, no data loss.
fn migrate_add_class_micromodule_columns(conn: &Connection) -> AppResult<()> {
    let has_table = |name: &str| -> AppResult<bool> {
        Ok(conn
            .query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", params![name], |_| Ok(()))
            .optional()?
            .is_some())
    };
    let has_column = |table: &str, column: &str| -> AppResult<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let names = stmt.query_map([], |row| row.get::<_, String>(1))?.collect::<Result<Vec<_>, _>>()?;
        Ok(names.iter().any(|name| name == column))
    };

    if has_table("classes")? && !has_column("classes", "hours")? {
        conn.execute("ALTER TABLE classes ADD COLUMN hours REAL NOT NULL DEFAULT 0", [])?;
    }
    // The per-micromodule learning objective shown in the plan and in the
    // class notebook's header — nullable on purpose: classes imported before
    // `Micromodule::objective` existed simply render no objective line.
    if has_table("classes")? && !has_column("classes", "objective")? {
        conn.execute("ALTER TABLE classes ADD COLUMN objective TEXT", [])?;
    }
    if has_table("notebook_documents")? && !has_column("notebook_documents", "pedagogical_rationale")? {
        conn.execute("ALTER TABLE notebook_documents ADD COLUMN pedagogical_rationale TEXT", [])?;
    }
    Ok(())
}

/// One-time migration: adds the per-block gating columns the iterative
/// mastery-gate redesign needs. `notebook_blocks.status` backfills existing
/// rows to `'passed'` (NOT the schema's own `'ready'` default, which only
/// applies to freshly inserted rows going forward) — every block persisted
/// by the retired whole-document `upsert_document_with_blocks` was, by
/// definition, already shown to the student as finished content, so it must
/// keep rendering unlocked/read-only exactly as before rather than suddenly
/// appearing locked. `notebook_documents.current_block_index` backfills to
/// each document's own block count (fully unlocked), computed per-document
/// since a flat `DEFAULT` can't do that — a document with fewer blocks than
/// another must not be left pointing past its own last block.
fn migrate_add_block_gating_columns(conn: &Connection) -> AppResult<()> {
    let has_table = |name: &str| -> AppResult<bool> {
        Ok(conn
            .query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1", params![name], |_| Ok(()))
            .optional()?
            .is_some())
    };
    let has_column = |table: &str, column: &str| -> AppResult<bool> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let names = stmt.query_map([], |row| row.get::<_, String>(1))?.collect::<Result<Vec<_>, _>>()?;
        Ok(names.iter().any(|name| name == column))
    };

    if has_table("notebook_blocks")? {
        if !has_column("notebook_blocks", "status")? {
            conn.execute("ALTER TABLE notebook_blocks ADD COLUMN status TEXT NOT NULL DEFAULT 'passed'", [])?;
        }
        if !has_column("notebook_blocks", "attempt_count")? {
            conn.execute("ALTER TABLE notebook_blocks ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0", [])?;
        }
        if !has_column("notebook_blocks", "last_feedback")? {
            conn.execute("ALTER TABLE notebook_blocks ADD COLUMN last_feedback TEXT", [])?;
        }
    }
    if has_table("notebook_documents")? {
        if !has_column("notebook_documents", "current_block_index")? {
            conn.execute("ALTER TABLE notebook_documents ADD COLUMN current_block_index INTEGER NOT NULL DEFAULT 0", [])?;
            // Backfill: every pre-existing document was generated whole and
            // shown in full — unlock it entirely (index = its block count).
            conn.execute(
                "UPDATE notebook_documents SET current_block_index = (
                    SELECT COUNT(*) FROM notebook_blocks WHERE notebook_blocks.document_id = notebook_documents.id
                 )",
                [],
            )?;
        }
        if !has_column("notebook_documents", "initial_prediction")? {
            conn.execute("ALTER TABLE notebook_documents ADD COLUMN initial_prediction TEXT", [])?;
        }
    }
    Ok(())
}

/// Full context needed to generate a class's notebook: the course it
/// belongs to, the syllabus week (milestone) it's part of, and the class
/// itself.
#[derive(Debug, Clone)]
pub struct ClassGenerationContext {
    pub course: Course,
    pub milestone: SyllabusMilestone,
    pub class: ClassRecord,
}

#[derive(Clone)]
pub struct NotebookStore {
    conn: Arc<Mutex<Connection>>,
}

impl NotebookStore {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Persistence(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> AppResult<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        // `CREATE TABLE IF NOT EXISTS` is a no-op against a table that
        // already exists with an OLDER shape — a real sqlite file created
        // before the fixed-4-section `notebook_blocks.section_type` column
        // was retired still has it as `NOT NULL`, so every insert (which no
        // longer supplies it) fails. Migrate that one known shape change
        // before trusting the schema.
        migrate_retired_section_type_column(&conn)?;
        migrate_add_diagnostic_battery_column(&conn)?;
        migrate_add_class_micromodule_columns(&conn)?;
        migrate_add_block_gating_columns(&conn)?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // --- Course / milestone / class -----------------------------------------

    pub fn create_course(&self, title: &str, target_goal: &str, total_weeks: u16) -> AppResult<Course> {
        let course = Course { id: Uuid::new_v4().to_string(), title: title.to_string(), target_goal: target_goal.to_string(), total_weeks, created_at_ms: now_ms() };
        self.lock().execute(
            "INSERT INTO courses (id, title, target_goal, total_weeks, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![course.id, course.title, course.target_goal, course.total_weeks, course.created_at_ms],
        )?;
        Ok(course)
    }

    pub fn create_milestone(
        &self,
        course_id: &str,
        week_number: u16,
        title: &str,
        deliverable: &str,
    ) -> AppResult<SyllabusMilestone> {
        let m = SyllabusMilestone {
            id: Uuid::new_v4().to_string(),
            course_id: course_id.to_string(),
            week_number,
            title: title.to_string(),
            deliverable: deliverable.to_string(),
        };
        // The `deliverable_goal` SQL column name is unchanged storage
        // bookkeeping (see `SCHEMA_SQL`) — only the Rust/TS-facing field was
        // renamed to `deliverable` to match `Milestone::deliverable`.
        self.lock().execute(
            "INSERT INTO syllabus_milestones (id, course_id, week_number, title, deliverable_goal) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![m.id, m.course_id, m.week_number, m.title, m.deliverable],
        )?;
        Ok(m)
    }

    pub fn create_class(&self, milestone_id: &str, class_number: u16, title: &str, order_index: u32, hours: f32) -> AppResult<ClassRecord> {
        let c = ClassRecord {
            id: Uuid::new_v4().to_string(),
            milestone_id: milestone_id.to_string(),
            class_number,
            title: title.to_string(),
            order_index,
            hours,
            objective: None,
            complete: false,
        };
        self.lock().execute(
            "INSERT INTO classes (id, milestone_id, class_number, title, order_index, hours) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![c.id, c.milestone_id, c.class_number, c.title, c.order_index, c.hours],
        )?;
        Ok(c)
    }

    /// Stamps a class with its micromodule's learning objective right after
    /// [`create_class`] — a separate call rather than a 6th parameter so the
    /// (many) tests that don't care about objectives keep their call sites.
    /// `objective` is `None`-tolerant: a syllabus sealed before
    /// `Micromodule::objective` existed just leaves the column NULL, which
    /// the UI renders as "no objective line".
    pub fn set_class_objective(&self, class_id: &str, objective: Option<&str>) -> AppResult<()> {
        self.lock()
            .execute("UPDATE classes SET objective = ?1 WHERE id = ?2", params![objective, class_id])?;
        Ok(())
    }

    /// Every course, newest first — what the frontend uses on startup to
    /// decide whether to land on the classes view instead of the roadmap.
    pub fn list_courses(&self) -> AppResult<Vec<Course>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, target_goal, total_weeks, created_at_ms
             FROM courses ORDER BY created_at_ms DESC, rowid DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Course {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    target_goal: row.get(2)?,
                    total_weeks: row.get(3)?,
                    created_at_ms: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Deletes a course AND everything hanging off it (milestones, classes,
    /// documents, blocks) — the FK chain is `ON DELETE CASCADE` with
    /// `foreign_keys=ON` (see [`Self::init`]), so removing the root row is
    /// the whole operation. The ownership side of that cascade lives in
    /// `RoadmapService::delete_session`: a session owns its course, so
    /// deleting the session deletes this too.
    pub fn delete_course(&self, course_id: &str) -> AppResult<()> {
        self.lock().execute("DELETE FROM courses WHERE id = ?1", params![course_id])?;
        Ok(())
    }

    /// Wipes a class's notebook (its `notebook_documents` row and, via the
    /// same `ON DELETE CASCADE` chain `delete_course` relies on, every block
    /// hanging off it) WITHOUT touching the class/milestone/course rows
    /// themselves — the manual "start this one class over" escape hatch for
    /// an atypical notebook (stuck on a bad composition, a corrupted block
    /// no single regeneration fixes, etc.), as opposed to
    /// `NotebookService::regenerate_block`'s in-place single-block repair.
    /// A no-op if the class never had a notebook generated yet.
    pub fn delete_notebook_for_class(&self, class_id: &str) -> AppResult<()> {
        self.lock().execute("DELETE FROM notebook_documents WHERE class_id = ?1", params![class_id])?;
        Ok(())
    }

    /// Removes every course that no session references anymore — the
    /// startup sweep for the "borré la sesión y las clases quedaron" bug.
    /// `owned_course_ids` is what the surviving roadmap sessions point at
    /// (`RoadmapSession::imported_course_id`); anything else in `courses`
    /// is an orphan from a session deleted before the cascade existed.
    /// Returns how many courses were removed (for logging/auditing).
    pub fn delete_courses_except(&self, owned_course_ids: &[String]) -> AppResult<usize> {
        let conn = self.lock();
        let mut removed = 0usize;
        {
            let mut stmt = conn.prepare("SELECT id FROM courses")?;
            let ids: Vec<String> = stmt.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;
            drop(stmt);
            for id in ids {
                if owned_course_ids.iter().any(|owned| owned == &id) {
                    continue;
                }
                removed += conn.execute("DELETE FROM courses WHERE id = ?1", params![id])?;
            }
        }
        Ok(removed)
    }

    /// Every class for a course, ordered by `order_index` — what the
    /// frontend uses to link "open notebook" per week right after import.
    pub fn list_classes_for_course(&self, course_id: &str) -> AppResult<Vec<ClassRecord>> {
        // Query under the lock, completion checks AFTER dropping it —
        // `is_class_complete` re-enters `self.lock()` and the mutex isn't
        // reentrant.
        let rows = {
            let conn = self.lock();
            let mut stmt = conn.prepare(
                "SELECT c.id, c.milestone_id, c.class_number, c.title, c.order_index, c.hours, c.objective
                 FROM classes c JOIN syllabus_milestones m ON m.id = c.milestone_id
                 WHERE m.course_id = ?1 ORDER BY c.order_index ASC",
            )?;
            let mapped = stmt.query_map(params![course_id], |row| {
                Ok(ClassRecord {
                    id: row.get(0)?,
                    milestone_id: row.get(1)?,
                    class_number: row.get(2)?,
                    title: row.get(3)?,
                    order_index: row.get(4)?,
                    hours: row.get(5)?,
                    objective: row.get(6)?,
                    complete: false,
                })
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let mut classes = rows;
        for c in &mut classes {
            c.complete = self.is_class_complete(&c.id)?;
        }
        Ok(classes)
    }

    /// Whether this class's notebook is "understood and practiced" — the
    /// derived `ClassRecord::complete` flag, computed per class from its
    /// persisted document + blocks (see [`crate::domain::notebook::class_is_complete`]).
    /// A class with no notebook yet is never complete.
    pub fn is_class_complete(&self, class_id: &str) -> AppResult<bool> {
        let Some(payload) = self.load_notebook_by_class(class_id)? else {
            return Ok(false);
        };
        Ok(crate::domain::notebook::class_is_complete(&payload.document, &payload.blocks))
    }

    /// Joined context for the notebook-generation agent: course + milestone
    /// + class in one shot, or `None` if `class_id` doesn't exist.
    pub fn class_generation_context(&self, class_id: &str) -> AppResult<Option<ClassGenerationContext>> {
        let conn = self.lock();
        conn.query_row(
            "SELECT
                co.id, co.title, co.target_goal, co.total_weeks, co.created_at_ms,
                m.id, m.course_id, m.week_number, m.title, m.deliverable_goal,
                cl.id, cl.milestone_id, cl.class_number, cl.title, cl.order_index, cl.hours, cl.objective
             FROM classes cl
             JOIN syllabus_milestones m ON m.id = cl.milestone_id
             JOIN courses co ON co.id = m.course_id
             WHERE cl.id = ?1",
            params![class_id],
            |row| {
                Ok(ClassGenerationContext {
                    course: Course {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        target_goal: row.get(2)?,
                        total_weeks: row.get(3)?,
                        created_at_ms: row.get(4)?,
                    },
                    milestone: SyllabusMilestone {
                        id: row.get(5)?,
                        course_id: row.get(6)?,
                        week_number: row.get(7)?,
                        title: row.get(8)?,
                        deliverable: row.get(9)?,
                    },
                    class: ClassRecord {
                        id: row.get(10)?,
                        milestone_id: row.get(11)?,
                        class_number: row.get(12)?,
                        title: row.get(13)?,
                        order_index: row.get(14)?,
                        hours: row.get(15)?,
                        objective: row.get(16)?,
                        complete: false,
                    },
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    /// How many of this course's ALREADY-GENERATED classes (excluding
    /// `exclude_class_id`, so regenerating a class doesn't count its own
    /// stale notebook) contain at least one block of each type, plus the
    /// total number of generated classes counted. Used by
    /// `NotebookService::generate_class_notebook` to push back when one
    /// block type (in practice, `heuristic_error_audit` — "find the error"
    /// — picked as a habitual default) comes to dominate a course instead
    /// of the sequence genuinely varying per topic.
    pub fn block_type_usage_for_course(&self, course_id: &str, exclude_class_id: &str) -> AppResult<(HashMap<String, i64>, i64)> {
        let conn = self.lock();
        let total_documents: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT nd.id)
             FROM notebook_documents nd
             JOIN classes cl ON cl.id = nd.class_id
             JOIN syllabus_milestones m ON m.id = cl.milestone_id
             WHERE m.course_id = ?1 AND cl.id != ?2",
            params![course_id, exclude_class_id],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT nb.block_type, COUNT(DISTINCT nb.document_id)
             FROM notebook_blocks nb
             JOIN notebook_documents nd ON nd.id = nb.document_id
             JOIN classes cl ON cl.id = nd.class_id
             JOIN syllabus_milestones m ON m.id = cl.milestone_id
             WHERE m.course_id = ?1 AND cl.id != ?2
             GROUP BY nb.block_type",
        )?;
        let counts = stmt
            .query_map(params![course_id, exclude_class_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
            .collect::<Result<HashMap<_, _>, _>>()?;

        Ok((counts, total_documents))
    }

    /// Persists the calibration battery generated alongside a course's
    /// syllabus (see `SyllabusExecutionArgs` in `domain::roadmap`) — called
    /// once, right after the course is created, with an empty `answers` map.
    pub fn set_course_diagnostic_battery(&self, course_id: &str, battery: &DiagnosticBattery) -> AppResult<()> {
        let state = DiagnosticBatteryState { battery: battery.clone(), answers: Default::default() };
        let json = serde_json::to_string(&state).map_err(|e| AppError::Persistence(e.to_string()))?;
        self.lock().execute("UPDATE courses SET diagnostic_battery_json = ?1 WHERE id = ?2", params![json, course_id])?;
        Ok(())
    }

    /// The course's diagnostic battery plus whatever the student has
    /// answered so far — what `RoadmapView`'s plan screen renders, and what
    /// `NotebookService::generate_class_notebook` reads back to personalize
    /// later classes. `None` if this course predates the battery feature or
    /// its syllabus generation never produced one.
    pub fn get_course_diagnostic_battery(&self, course_id: &str) -> AppResult<Option<DiagnosticBatteryState>> {
        let json: Option<String> = self
            .lock()
            .query_row("SELECT diagnostic_battery_json FROM courses WHERE id = ?1", params![course_id], |row| row.get(0))
            .optional()?
            .flatten();
        Ok(match json {
            Some(j) => serde_json::from_str(&j).ok(),
            None => None,
        })
    }

    /// Replaces the course's diagnostic battery answers wholesale — the
    /// frontend always sends its full current answers map (same pattern as
    /// notebook blocks' `selectedOption`/`answers`), so a merge isn't
    /// needed. No-op (not an error) if the course has no battery on record.
    pub fn save_diagnostic_battery_answers(&self, course_id: &str, answers: &HashMap<String, String>) -> AppResult<()> {
        let Some(mut state) = self.get_course_diagnostic_battery(course_id)? else { return Ok(()) };
        state.answers = answers.clone();
        let json = serde_json::to_string(&state).map_err(|e| AppError::Persistence(e.to_string()))?;
        self.lock().execute("UPDATE courses SET diagnostic_battery_json = ?1 WHERE id = ?2", params![json, course_id])?;
        Ok(())
    }

    // --- Learner cognitive memory --------------------------------------------

    /// Loads `learner_id`'s cognitive memory, or a fresh default if none was
    /// ever saved (first-ever generation for this learner) or the stored
    /// JSON somehow fails to parse (a schema change on a dev build, say) —
    /// personalization degrading to neutral defaults is always preferable to
    /// failing the whole class-generation call over stale memory state.
    pub fn get_learner_memory(&self, learner_id: &str) -> AppResult<LearnerCognitiveMemory> {
        let json: Option<String> =
            self.lock().query_row("SELECT data_json FROM learner_memory WHERE learner_id = ?1", params![learner_id], |row| row.get(0)).optional()?;
        Ok(match json {
            Some(j) => serde_json::from_str(&j).unwrap_or_else(|_| LearnerCognitiveMemory::new(learner_id)),
            None => LearnerCognitiveMemory::new(learner_id),
        })
    }

    pub fn save_learner_memory(&self, memory: &LearnerCognitiveMemory) -> AppResult<()> {
        let json = serde_json::to_string(memory).map_err(|e| AppError::Persistence(e.to_string()))?;
        self.lock().execute(
            "INSERT INTO learner_memory (learner_id, data_json, updated_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(learner_id) DO UPDATE SET data_json = excluded.data_json, updated_at_ms = excluded.updated_at_ms",
            params![memory.learner_id, json, now_ms()],
        )?;
        Ok(())
    }

    // --- Notebook documents / blocks ----------------------------------------

    /// Returns the existing notebook document for `class_id` (whatever
    /// progress it's at — 0 blocks, mid-sequence, or finished), or creates a
    /// fresh empty one (`status = Draft`, `current_block_index = 0`, no
    /// blocks yet) if this is the first time the class is being opened.
    /// Replaces the old whole-document `upsert_document_with_blocks`: blocks
    /// are now inserted one at a time via `insert_block` as generation
    /// produces them, never all at once.
    pub fn ensure_document_shell(&self, class_id: &str, class_title: &str) -> AppResult<NotebookDocument> {
        let conn = self.lock();
        if let Some(existing) = conn
            .query_row(
                "SELECT id, class_id, title, status, updated_at_ms, pedagogical_rationale, current_block_index, initial_prediction
                 FROM notebook_documents WHERE class_id = ?1",
                params![class_id],
                row_to_document,
            )
            .optional()?
        {
            return Ok(existing);
        }
        let doc = NotebookDocument {
            id: Uuid::new_v4().to_string(),
            class_id: class_id.to_string(),
            title: class_title.to_string(),
            status: NotebookStatus::Draft,
            updated_at_ms: now_ms(),
            pedagogical_rationale: None,
            current_block_index: 0,
            initial_prediction: None,
        };
        conn.execute(
            "INSERT INTO notebook_documents (id, class_id, title, status, updated_at_ms, pedagogical_rationale, current_block_index, initial_prediction)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                doc.id,
                doc.class_id,
                doc.title,
                doc.status.as_str(),
                doc.updated_at_ms,
                doc.pedagogical_rationale,
                doc.current_block_index,
                doc.initial_prediction
            ],
        )?;
        Ok(doc)
    }

    /// Persists ONE freshly generated block at the end of `document_id`'s
    /// sequence — `order_index` is `self.block_count(document_id)`, computed
    /// inside the same transaction so two concurrent inserts can never
    /// collide on the same index. Also marks the document `Ready` (there's
    /// now at least one block to show) if it was still `Draft`. This is the
    /// only block-creation path — there is no "pending" placeholder row.
    pub fn insert_block(
        &self,
        document_id: &str,
        block_type: DynamicBlockType,
        content_json: &serde_json::Value,
        status: BlockStatus,
    ) -> AppResult<NotebookBlock> {
        self.insert_block_if_count(document_id, block_type, content_json, status, None)?
            .ok_or_else(|| AppError::Persistence(format!("el conteo de bloques cambió al insertar en {document_id}")))
    }

    /// Same append as [`insert_block`], optionally guarded by the block
    /// count the caller READ BEFORE spending seconds generating the block.
    /// When the count no longer matches, another writer won the race (React
    /// StrictMode's double-mount fires two `start_class_notebook` calls;
    /// parallel background chains do the same) and this insert is
    /// discarded: `Ok(None)` means "this candidate lost — do not spawn a
    /// chain, do not advance the cursor, return the winner's progress".
    /// Count check + insert share ONE transaction, so the guard is atomic.
    pub fn insert_block_if_count(
        &self,
        document_id: &str,
        block_type: DynamicBlockType,
        content_json: &serde_json::Value,
        status: BlockStatus,
        expected_block_count: Option<u32>,
    ) -> AppResult<Option<NotebookBlock>> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let order_index: u32 =
            tx.query_row("SELECT COUNT(*) FROM notebook_blocks WHERE document_id = ?1", params![document_id], |r| r.get(0))?;
        if expected_block_count.is_some_and(|expected| order_index != expected) {
            return Ok(None);
        }
        let block_id = Uuid::new_v4().to_string();
        let content_text = serde_json::to_string(content_json).map_err(|e| AppError::Persistence(e.to_string()))?;
        tx.execute(
            "INSERT INTO notebook_blocks (id, document_id, block_type, content_json, order_index, status, attempt_count, last_feedback)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL)",
            params![block_id, document_id, block_type.as_str(), content_text, order_index, status.as_str()],
        )?;
        tx.execute(
            "UPDATE notebook_documents SET status = ?1, updated_at_ms = ?2 WHERE id = ?3",
            params![NotebookStatus::Ready.as_str(), now_ms(), document_id],
        )?;
        tx.commit()?;
        Ok(Some(NotebookBlock {
            id: block_id,
            document_id: document_id.to_string(),
            block_type,
            content_json: content_json.clone(),
            order_index,
            status,
            attempt_count: 0,
            last_feedback: None,
        }))
    }

    /// Updates a graded block's status/feedback in place. `attempt_increment`
    /// is `true` on every graded submission (pass or fail) — attempt count
    /// tracks total tries, used to trigger escalation at 3 failures.
    pub fn update_block_status(
        &self,
        block_id: &str,
        status: BlockStatus,
        feedback: Option<&str>,
        attempt_increment: bool,
    ) -> AppResult<NotebookBlock> {
        let conn = self.lock();
        if attempt_increment {
            conn.execute(
                "UPDATE notebook_blocks SET status = ?1, last_feedback = ?2, attempt_count = attempt_count + 1 WHERE id = ?3",
                params![status.as_str(), feedback, block_id],
            )?;
        } else {
            conn.execute(
                "UPDATE notebook_blocks SET status = ?1, last_feedback = ?2 WHERE id = ?3",
                params![status.as_str(), feedback, block_id],
            )?;
        }
        conn.query_row(
            "SELECT id, document_id, block_type, content_json, order_index, status, attempt_count, last_feedback
             FROM notebook_blocks WHERE id = ?1",
            params![block_id],
            row_to_block,
        )
        .map_err(AppError::from)
    }

    /// Advances (or otherwise sets) the document's gate cursor — the literal
    /// "furthest unlocked block" pointer. Also bumps `updated_at_ms`.
    pub fn set_document_current_index(&self, document_id: &str, index: u32) -> AppResult<()> {
        self.lock().execute(
            "UPDATE notebook_documents SET current_block_index = ?1, updated_at_ms = ?2 WHERE id = ?3",
            params![index, now_ms(), document_id],
        )?;
        Ok(())
    }

    /// Monotonic (SQL `MAX`) cursor advance — the ONLY cursor writer the
    /// generation paths use. Every caller derives an ABSOLUTE target from
    /// the block it just inserted, so a writer holding a stale read can
    /// never move the cursor backwards or skip a gate; the old relative
    /// `current + 1` (a non-atomic get-then-set) silently lost races
    /// between concurrent chains and wedged the notebook.
    pub fn advance_cursor_to_at_least(&self, document_id: &str, target: u32) -> AppResult<()> {
        self.lock().execute(
            "UPDATE notebook_documents SET current_block_index = MAX(current_block_index, ?2), updated_at_ms = ?3 WHERE id = ?1",
            params![document_id, target, now_ms()],
        )?;
        Ok(())
    }

    /// Records the student's first-ever gate answer for this document,
    /// verbatim. A no-op if one is already recorded — this field is written
    /// exactly once per notebook, by design (see
    /// `NotebookDocument::initial_prediction`).
    pub fn set_document_initial_prediction(&self, document_id: &str, value: &str) -> AppResult<()> {
        self.lock().execute(
            "UPDATE notebook_documents SET initial_prediction = ?1 WHERE id = ?2 AND initial_prediction IS NULL",
            params![value, document_id],
        )?;
        Ok(())
    }

    pub fn get_document(&self, document_id: &str) -> AppResult<Option<NotebookDocument>> {
        self.lock()
            .query_row(
                "SELECT id, class_id, title, status, updated_at_ms, pedagogical_rationale, current_block_index, initial_prediction
                 FROM notebook_documents WHERE id = ?1",
                params![document_id],
                row_to_document,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn get_block(&self, block_id: &str) -> AppResult<Option<NotebookBlock>> {
        self.lock()
            .query_row(
                "SELECT id, document_id, block_type, content_json, order_index, status, attempt_count, last_feedback
                 FROM notebook_blocks WHERE id = ?1",
                params![block_id],
                row_to_block,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn load_notebook_by_class(&self, class_id: &str) -> AppResult<Option<NotebookPayload>> {
        let conn = self.lock();
        let document = conn
            .query_row(
                "SELECT id, class_id, title, status, updated_at_ms, pedagogical_rationale, current_block_index, initial_prediction
                 FROM notebook_documents WHERE class_id = ?1",
                params![class_id],
                row_to_document,
            )
            .optional()?;
        let Some(document) = document else { return Ok(None) };
        let blocks = Self::load_blocks(&conn, &document.id)?;
        Ok(Some(NotebookPayload { document, blocks }))
    }

    fn load_blocks(conn: &Connection, document_id: &str) -> AppResult<Vec<NotebookBlock>> {
        let mut stmt = conn.prepare(
            "SELECT id, document_id, block_type, content_json, order_index, status, attempt_count, last_feedback
             FROM notebook_blocks WHERE document_id = ?1 ORDER BY order_index ASC",
        )?;
        let rows = stmt.query_map(params![document_id], row_to_block)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Applies learner edits to existing blocks (answered a prediction gate,
    /// edited rich text, completed a sandbox). Silently skips any update
    /// whose `id` doesn't belong to `notebook_id` — defensive against a
    /// stale/forged id, never lets one notebook's edits leak into another's.
    pub fn update_block_contents(&self, notebook_id: &str, updates: &[BlockUpdate]) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for u in updates {
            let content_text = serde_json::to_string(&u.content_json).map_err(|e| AppError::Persistence(e.to_string()))?;
            tx.execute(
                "UPDATE notebook_blocks SET content_json = ?1
                 WHERE id = ?2 AND document_id = ?3",
                params![content_text, u.id, notebook_id],
            )?;
        }
        tx.execute("UPDATE notebook_documents SET updated_at_ms = ?1 WHERE id = ?2", params![now_ms(), notebook_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Replaces ONE block's payload in place — the "regenerate this broken
    /// block" path. Keeps `id`, `order_index` and `document_id`, so the gate
    /// cursor, the pacing and the student's position in the class are all
    /// untouched (blocks only ever get appended, so re-inserting the
    /// replacement would push it past every gate behind it). A block that had
    /// already been graded `Passed`/`Escalated` keeps that status — the grade
    /// stands regardless of the new content — while anything else resets to
    /// `Ready` with a clean attempt counter and no stale feedback, so the
    /// student meets a fresh, gradeable block. Errors with `InvalidInput`
    /// when `block_id` doesn't exist.
    pub fn replace_block_payload(
        &self,
        block_id: &str,
        block_type: DynamicBlockType,
        content_json: &serde_json::Value,
    ) -> AppResult<NotebookBlock> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let (document_id, previous_status): (String, String) = tx
            .query_row(
                "SELECT document_id, status FROM notebook_blocks WHERE id = ?1",
                params![block_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| AppError::InvalidInput(format!("no existe el bloque {block_id}")))?;
        let graded = matches!(
            BlockStatus::parse(&previous_status),
            Some(BlockStatus::Passed) | Some(BlockStatus::Escalated)
        );
        let (status, attempts, feedback) = if graded {
            (
                BlockStatus::parse(&previous_status).unwrap_or(BlockStatus::Passed),
                None::<i64>,
                None::<&str>,
            )
        } else {
            (BlockStatus::Ready, Some(0i64), None)
        };
        let content_text = serde_json::to_string(content_json).map_err(|e| AppError::Persistence(e.to_string()))?;
        tx.execute(
            "UPDATE notebook_blocks SET block_type = ?1, content_json = ?2, status = ?3,
                                        attempt_count = COALESCE(?4, attempt_count), last_feedback = ?5
             WHERE id = ?6",
            params![block_type.as_str(), content_text, status.as_str(), attempts, feedback, block_id],
        )?;
        tx.execute(
            "UPDATE notebook_documents SET updated_at_ms = ?1 WHERE id = ?2",
            params![now_ms(), document_id],
        )?;
        let block = tx.query_row(
            "SELECT id, document_id, block_type, content_json, order_index, status, attempt_count, last_feedback
             FROM notebook_blocks WHERE id = ?1",
            params![block_id],
            row_to_block,
        )?;
        tx.commit()?;
        Ok(block)
    }
}

fn row_to_document(row: &rusqlite::Row) -> rusqlite::Result<NotebookDocument> {
    let status_str: String = row.get(3)?;
    Ok(NotebookDocument {
        id: row.get(0)?,
        class_id: row.get(1)?,
        title: row.get(2)?,
        status: NotebookStatus::parse(&status_str).unwrap_or(NotebookStatus::Draft),
        updated_at_ms: row.get(4)?,
        pedagogical_rationale: row.get(5)?,
        current_block_index: row.get(6)?,
        initial_prediction: row.get(7)?,
    })
}

fn row_to_block(row: &rusqlite::Row) -> rusqlite::Result<NotebookBlock> {
    let block_str: String = row.get(2)?;
    let content_text: String = row.get(3)?;
    let content_json = serde_json::from_str(&content_text).unwrap_or(serde_json::Value::Null);
    let status_str: String = row.get(5)?;
    Ok(NotebookBlock {
        id: row.get(0)?,
        document_id: row.get(1)?,
        block_type: DynamicBlockType::parse(&block_str).unwrap_or(DynamicBlockType::AnchoredMicroTheory),
        content_json,
        order_index: row.get(4)?,
        status: BlockStatus::parse(&status_str).unwrap_or(BlockStatus::Passed),
        attempt_count: row.get(6)?,
        last_feedback: row.get(7)?,
    })
}

/// Seeds `courses`/`syllabus_milestones`/`classes` from a confirmed roadmap
/// syllabus — one class per MICROMODULE now, not per week (each micromodule
/// is already a bounded ≤4h unit — see `domain::roadmap::Micromodule` and
/// `application::roadmap_service::grounding::MAX_SESSION_HOURS`), each
/// class carrying that micromodule's own `hours`. Shared by
/// `NotebookService::import_course_from_roadmap` and `RoadmapService`'s
/// seal-time `import_syllabus_skeleton`, which both used to duplicate this
/// loop independently — sharing it here is the same precedent as
/// `dynamic_notebook_json_schema` being the one function both tool-calling
/// contracts call, so the two import paths can't drift apart. Returns
/// `(course_id, first_class_id, first_class_title)` — `first_class_id` is
/// `None` only if the syllabus somehow has zero micromodules total.
pub fn import_syllabus_into_store(
    store: &NotebookStore,
    syllabus: &crate::domain::roadmap::RoadmapSyllabusPackage,
    target_goal: &str,
) -> AppResult<(String, Option<String>, String)> {
    let course = store.create_course(&syllabus.course_title, target_goal, syllabus.total_weeks)?;
    let mut first_class_id: Option<String> = None;
    let mut first_class_title = syllabus.course_title.clone();
    let mut order_index: u32 = 0;
    for m in &syllabus.milestones {
        let milestone = store.create_milestone(&course.id, m.week, &m.title, &m.deliverable)?;
        for (i, module) in m.micromodules.iter().enumerate() {
            let class_title = format!("Semana {}: {}", m.week, module.label);
            let class = store.create_class(&milestone.id, (i + 1) as u16, &class_title, order_index, module.hours)?;
            store.set_class_objective(&class.id, module.objective.as_deref())?;
            order_index += 1;
            if first_class_id.is_none() {
                first_class_id = Some(class.id.clone());
                first_class_title = class.title.clone();
            }
        }
    }
    Ok((course.id, first_class_id, first_class_title))
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests;
