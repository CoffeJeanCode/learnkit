    use super::*;
    use crate::domain::notebook::{GeneratedSectionBlock, PredictionComparison};

    /// Regression test: a real sqlite file written before the fixed-4-
    /// section model was retired still had `notebook_blocks.section_type
    /// TEXT NOT NULL` — every insert since (which stopped supplying it)
    /// failed with `NOT NULL constraint failed: notebook_blocks.
    /// section_type`. `NotebookStore::open`/`init` must migrate that shape
    /// away transparently, on the very next startup, with no manual file
    /// deletion required.
    #[test]
    fn opening_a_store_with_the_retired_section_type_column_migrates_it_away() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "CREATE TABLE courses (id TEXT PRIMARY KEY, title TEXT, target_goal TEXT, total_weeks INTEGER, created_at_ms INTEGER);
             CREATE TABLE syllabus_milestones (id TEXT PRIMARY KEY, course_id TEXT, week_number INTEGER, title TEXT, deliverable_goal TEXT);
             CREATE TABLE classes (id TEXT PRIMARY KEY, milestone_id TEXT, class_number INTEGER, title TEXT, order_index INTEGER);
             CREATE TABLE notebook_documents (id TEXT PRIMARY KEY, class_id TEXT UNIQUE, title TEXT, status TEXT, updated_at_ms INTEGER);
             CREATE TABLE notebook_blocks (
                 id TEXT PRIMARY KEY,
                 document_id TEXT NOT NULL,
                 section_type TEXT NOT NULL,
                 block_type TEXT NOT NULL,
                 content_json TEXT NOT NULL,
                 order_index INTEGER NOT NULL
             );
             INSERT INTO notebook_documents (id, class_id, title, status, updated_at_ms) VALUES ('doc1', 'class1', 'Clase 1', 'ready', 0);
             INSERT INTO notebook_blocks (id, document_id, section_type, block_type, content_json, order_index)
                 VALUES ('b1', 'doc1', 'theory', 'tiptap_rich_text', '{}', 0);",
        )
        .expect("seed the old (pre-migration) schema");

        let store = NotebookStore::init(conn).expect("init migrates the stale schema instead of failing");

        // The stale document/blocks are gone (their content was agent-
        // generated and regenerates on demand) — but writing a NEW notebook
        // under the current schema must now succeed, which is the actual
        // bug: it didn't, because `section_type` was still `NOT NULL` on
        // the live table underneath a no-op `CREATE TABLE IF NOT EXISTS`.
        let course = store.create_course("Tema", "Meta", 1).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");
        let doc = store.ensure_document_shell(&c.id, "Clase 1").expect("shell");
        let inserted = insert_sample_blocks(&store, &doc.id);
        assert_eq!(inserted.len(), 2);
    }

    /// Regression test: a `classes`/`notebook_documents` table created before
    /// one-class-per-micromodule (and the free-text rationale field) is
    /// missing `hours`/`pedagogical_rationale` — `CREATE TABLE IF NOT
    /// EXISTS` is a no-op against it, so `migrate_add_class_micromodule_
    /// columns` must add both columns directly instead of every write since
    /// failing with "no such column".
    #[test]
    fn opening_a_store_missing_hours_and_rationale_columns_migrates_them_in() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "CREATE TABLE courses (id TEXT PRIMARY KEY, title TEXT, target_goal TEXT, total_weeks INTEGER, created_at_ms INTEGER, diagnostic_battery_json TEXT);
             CREATE TABLE syllabus_milestones (id TEXT PRIMARY KEY, course_id TEXT, week_number INTEGER, title TEXT, deliverable_goal TEXT);
             CREATE TABLE classes (id TEXT PRIMARY KEY, milestone_id TEXT, class_number INTEGER, title TEXT, order_index INTEGER);
             CREATE TABLE notebook_documents (id TEXT PRIMARY KEY, class_id TEXT UNIQUE, title TEXT, status TEXT, updated_at_ms INTEGER);
             CREATE TABLE notebook_blocks (id TEXT PRIMARY KEY, document_id TEXT NOT NULL, block_type TEXT NOT NULL, content_json TEXT NOT NULL, order_index INTEGER NOT NULL);",
        )
        .expect("seed the pre-micromodule schema");

        let store = NotebookStore::init(conn).expect("init migrates the missing columns instead of failing");

        let course = store.create_course("Tema", "Meta", 1).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Semana 1: Módulo 1", 0, 4.0).expect("class carries hours now");
        assert_eq!(c.hours, 4.0);
        let doc = store.ensure_document_shell(&c.id, "Semana 1: Módulo 1").expect("shell carries no rationale yet");
        assert_eq!(doc.pedagogical_rationale, None);
        assert_eq!(doc.current_block_index, 0);
    }

    fn sample_blocks() -> Vec<GeneratedSectionBlock> {
        vec![
            GeneratedSectionBlock::AnchoredMicroTheory {
                title: "Título".to_string(),
                intuitive_hook: "Analogía".to_string(),
                system_rule: "Explicación breve".to_string(),
                frequent_error: "Error típico".to_string(),
            },
            GeneratedSectionBlock::MetacognitiveClosure {
                synthesis_task: "Compara ambos enfoques".to_string(),
                prediction_comparison: PredictionComparison {
                    initial_prediction: "ip".to_string(),
                    final_result: "fr".to_string(),
                    contrast_narrative: "cn".to_string(),
                },
                self_evaluation_checklist: vec!["Entiendo X".to_string()],
            },
        ]
    }

    /// Inserts `sample_blocks()` one at a time via `insert_block`, mirroring
    /// how `notebook_service::generation` persists blocks as they're
    /// generated — the store no longer accepts a whole batch in one call.
    fn insert_sample_blocks(store: &NotebookStore, document_id: &str) -> Vec<NotebookBlock> {
        sample_blocks()
            .iter()
            .map(|b| {
                let content_json = serde_json::to_value(b).expect("serializes");
                store.insert_block(document_id, b.block_type(), &content_json, BlockStatus::Ready).expect("insert")
            })
            .collect()
    }

    #[test]
    fn insert_block_if_count_discards_the_candidate_that_lost_the_race() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");
        let doc = store.ensure_document_shell(&c.id, "Clase 1").expect("shell");
        let sample = sample_blocks();

        let first_json = serde_json::to_value(&sample[0]).expect("serializes");
        let first = store
            .insert_block_if_count(&doc.id, sample[0].block_type(), &first_json, BlockStatus::Ready, Some(0))
            .expect("insert")
            .expect("the first writer wins");
        assert_eq!(first.order_index, 0);

        // A second writer read count=0 before generating the SAME first
        // block (StrictMode double-mount / parallel chain) — its candidate
        // must be discarded, not appended as a duplicate.
        let rejected = store
            .insert_block_if_count(&doc.id, sample[1].block_type(), &first_json, BlockStatus::Ready, Some(0))
            .expect("a lost race is not an error");
        assert!(rejected.is_none(), "stale candidate must be discarded");

        let loaded = store.load_notebook_by_class(&c.id).expect("load").expect("present");
        assert_eq!(loaded.blocks.len(), 1, "exactly one block persisted, no duplicate");

        // Matching count proceeds normally.
        let second_json = serde_json::to_value(&sample[1]).expect("serializes");
        let second = store
            .insert_block_if_count(&doc.id, sample[1].block_type(), &second_json, BlockStatus::Ready, Some(1))
            .expect("insert")
            .expect("matching count appends");
        assert_eq!(second.order_index, 1);
    }

    #[test]
    fn advance_cursor_to_at_least_never_moves_the_cursor_backwards() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");
        let doc = store.ensure_document_shell(&c.id, "Clase 1").expect("shell");

        store.advance_cursor_to_at_least(&doc.id, 3).expect("advance");
        store.advance_cursor_to_at_least(&doc.id, 1).expect("a stale writer runs anyway");
        let reloaded = store.get_document(&doc.id).expect("get").expect("present");
        assert_eq!(reloaded.current_block_index, 3, "SQL MAX keeps the high-water mark");

        store.advance_cursor_to_at_least(&doc.id, 5).expect("later advance");
        let reloaded = store.get_document(&doc.id).expect("get").expect("present");
        assert_eq!(reloaded.current_block_index, 5, "a forward advance still applies");
    }

    #[test]
    fn course_milestone_class_chain_persists_and_lists_in_order() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Ciclo de Krebs", "Aprobar el examen", 6).expect("course");
        let m2 = store.create_milestone(&course.id, 2, "Semana 2", "Entregable 2").expect("m2");
        let m1 = store.create_milestone(&course.id, 1, "Semana 1", "Entregable 1").expect("m1");
        store.create_class(&m2.id, 1, "Clase semana 2", 1, 3.0).expect("class2");
        store.create_class(&m1.id, 1, "Clase semana 1", 0, 3.0).expect("class1");

        let classes = store.list_classes_for_course(&course.id).expect("list");
        assert_eq!(classes.len(), 2);
        assert_eq!(classes[0].title, "Clase semana 1", "ordered by order_index, not insertion order");
        assert_eq!(classes[1].title, "Clase semana 2");
    }

    #[test]
    fn class_generation_context_joins_course_milestone_class() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.5).expect("class");

        let ctx = store.class_generation_context(&c.id).expect("query").expect("found");
        assert_eq!(ctx.course.id, course.id);
        assert_eq!(ctx.milestone.id, m.id);
        assert_eq!(ctx.class.id, c.id);
        assert_eq!(ctx.class.hours, 3.5);

        assert!(store.class_generation_context("missing").expect("query").is_none());
    }

    #[test]
    fn ensure_document_shell_is_idempotent_and_insert_block_appends_in_order() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");

        let first_shell = store.ensure_document_shell(&c.id, "Clase 1").expect("first shell");
        assert_eq!(first_shell.status, NotebookStatus::Draft, "no blocks yet");
        let second_shell = store.ensure_document_shell(&c.id, "Clase 1").expect("second call");
        assert_eq!(second_shell.id, first_shell.id, "idempotent — reuses the same document");

        let inserted = insert_sample_blocks(&store, &first_shell.id);
        assert_eq!(inserted.len(), 2);
        assert_eq!(inserted[0].order_index, 0);
        assert_eq!(inserted[1].order_index, 1);

        let loaded = store.load_notebook_by_class(&c.id).expect("load").expect("present");
        assert_eq!(loaded.blocks.len(), 2);
        assert_eq!(loaded.document.status, NotebookStatus::Ready, "at least one block now exists");
    }

    #[test]
    fn update_block_status_grades_a_block_and_bumps_attempt_count() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");
        let doc = store.ensure_document_shell(&c.id, "Clase 1").expect("shell");
        let blocks = insert_sample_blocks(&store, &doc.id);
        let block_id = blocks[0].id.clone();

        let failed = store.update_block_status(&block_id, BlockStatus::Failed, Some("pista"), true).expect("grade fail");
        assert_eq!(failed.status, BlockStatus::Failed);
        assert_eq!(failed.attempt_count, 1);
        assert_eq!(failed.last_feedback.as_deref(), Some("pista"));

        let passed = store.update_block_status(&block_id, BlockStatus::Passed, None, true).expect("grade pass");
        assert_eq!(passed.status, BlockStatus::Passed);
        assert_eq!(passed.attempt_count, 2);
    }

    #[test]
    fn set_document_current_index_and_initial_prediction_are_write_once_where_expected() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");
        let doc = store.ensure_document_shell(&c.id, "Clase 1").expect("shell");

        store.set_document_current_index(&doc.id, 1).expect("advance cursor");
        let reloaded = store.get_document(&doc.id).expect("get").expect("present");
        assert_eq!(reloaded.current_block_index, 1);

        store.set_document_initial_prediction(&doc.id, "primera respuesta").expect("capture");
        store.set_document_initial_prediction(&doc.id, "segunda respuesta").expect("no-op, already set");
        let reloaded = store.get_document(&doc.id).expect("get").expect("present");
        assert_eq!(reloaded.initial_prediction.as_deref(), Some("primera respuesta"), "write-once — never overwritten");
    }

    #[test]
    fn load_notebook_by_class_returns_none_when_never_generated() {
        let store = NotebookStore::open_in_memory().expect("open");
        assert!(store.load_notebook_by_class("ghost").expect("query").is_none());
    }

    #[test]
    fn list_courses_returns_newest_first() {
        let store = NotebookStore::open_in_memory().expect("open");
        assert!(store.list_courses().expect("empty").is_empty());
        let first = store.create_course("Primero", "Meta", 4).expect("course");
        let second = store.create_course("Segundo", "Meta", 4).expect("course");
        let listed = store.list_courses().expect("list");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id, "newest first");
        assert_eq!(listed[1].id, first.id);
    }

    #[test]
    fn update_block_contents_edits_in_place_and_ignores_foreign_ids() {
        let store = NotebookStore::open_in_memory().expect("open");
        let course = store.create_course("Tema", "Meta", 4).expect("course");
        let m = store.create_milestone(&course.id, 1, "Semana 1", "Entrega 1").expect("m");
        let c = store.create_class(&m.id, 1, "Clase 1", 0, 3.0).expect("class");
        let doc = store.ensure_document_shell(&c.id, "Clase 1").expect("shell");
        let generated = insert_sample_blocks(&store, &doc.id);
        let block_id = generated[0].id.clone();

        store
            .update_block_contents(
                &doc.id,
                &[
                    BlockUpdate { id: block_id.clone(), content_json: serde_json::json!({"markdown": "# Editado"}) },
                    BlockUpdate { id: "not-in-this-doc".to_string(), content_json: serde_json::json!({"x": 1}) },
                ],
            )
            .expect("update");

        let reloaded = store.load_notebook_by_class(&c.id).expect("load").expect("present");
        let edited = reloaded.blocks.iter().find(|b| b.id == block_id).expect("block still there");
        assert_eq!(edited.content_json, serde_json::json!({"markdown": "# Editado"}));
    }
