    use super::*;

    #[test]
    fn block_type_round_trips_through_its_wire_string() {
        for b in [
            DynamicBlockType::SpacedInterleavedRetrieval,
            DynamicBlockType::AnchoredMicroTheory,
            DynamicBlockType::DeclarativeVisualDiagram,
            DynamicBlockType::BranchingScenarioChallenge,
            DynamicBlockType::HeuristicErrorAudit,
            DynamicBlockType::InteractivePredictionGate,
            DynamicBlockType::HandsOnMission,
            DynamicBlockType::MetacognitiveClosure,
        ] {
            assert_eq!(DynamicBlockType::parse(b.as_str()), Some(b));
        }
        assert_eq!(DynamicBlockType::parse("nope"), None);
    }

    #[test]
    fn anchored_micro_theory_parses_camel_case_wire_shape() {
        let json = serde_json::json!({
            "blockType": "anchored_micro_theory",
            "title": "El bucle for",
            "intuitiveHook": "Piensa en una receta que repites N veces",
            "systemRule": "Un bucle for repite un bloque de código...",
            "frequentError": "Confundir la condición de corte con la de entrada"
        });
        let parsed: GeneratedSectionBlock = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.block_type(), DynamicBlockType::AnchoredMicroTheory);
    }

    #[test]
    fn declarative_visual_diagram_parses_camel_case_wire_shape() {
        let json = serde_json::json!({
            "blockType": "declarative_visual_diagram",
            "title": "El flujo del bucle",
            "visualAid": {
                "renderEngine": "mermaid",
                "chartType": "flowchart",
                "code": "graph TD; A-->B;",
                "caption": "Flujo del bucle"
            },
            "guidedWalkthrough": [{"stepNumber": 1, "targetVisualElement": "A", "pedagogicalInsight": "punto de entrada"}]
        });
        let parsed: GeneratedSectionBlock = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.block_type(), DynamicBlockType::DeclarativeVisualDiagram);
        match parsed {
            GeneratedSectionBlock::DeclarativeVisualDiagram { visual_aid, .. } => {
                assert!(matches!(visual_aid, StaticVisualSpec::Mermaid { .. }));
            }
            other => panic!("expected DeclarativeVisualDiagram, got {other:?}"),
        }
    }

    #[test]
    fn branching_scenario_challenge_parses_camel_case_wire_shape() {
        let json = serde_json::json!({
            "blockType": "branching_scenario_challenge",
            "scenario": "Tu equipo entrega tarde",
            "decisionPoint": "¿Qué haces primero?",
            "branches": [
                {"choice": "Añadir gente", "consequence": "Retrasa aún más (ley de Brooks)", "isOptimal": false},
                {"choice": "Recortar alcance", "consequence": "Entregas a tiempo lo esencial", "isOptimal": true}
            ]
        });
        let parsed: GeneratedSectionBlock = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.block_type(), DynamicBlockType::BranchingScenarioChallenge);
    }

    #[test]
    fn diagnostic_battery_parses_camel_case_wire_shape() {
        let json = serde_json::json!({
            "goalAlignment": "Mide si ya distingue difusión simple de transporte activo antes del examen",
            "questions": [
                {
                    "dimension": "intuition",
                    "prompt": "Si dejas caer tinta en agua quieta, ¿qué pasa con el tiempo?",
                    "options": ["Se esparce sola", "Se queda en un punto", "Se hunde y desaparece"],
                    "correctOption": "Se esparce sola",
                    "diagnosticInsight": "Elegir otra opción sugiere que no conecta el fenómeno cotidiano con el movimiento a favor de gradiente"
                },
                {
                    "dimension": "mechanics",
                    "prompt": "Si sube la concentración fuera de la célula, ¿qué le pasa a la difusión hacia adentro?",
                    "options": ["Aumenta", "Disminuye", "No cambia"],
                    "correctOption": "Aumenta",
                    "diagnosticInsight": "Elegir otra opción sugiere que no relaciona gradiente con velocidad de difusión"
                },
                {
                    "dimension": "critical_case",
                    "prompt": "Un estudiante dice que el transporte activo también sigue el gradiente. ¿Está bien?",
                    "options": ["Sí", "No, va en contra del gradiente y usa energía", "Solo a veces"],
                    "correctOption": "No, va en contra del gradiente y usa energía",
                    "diagnosticInsight": "Elegir otra opción revela la confusión más común del examen"
                }
            ]
        });
        let parsed: DiagnosticBattery = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.questions.len(), 3);
    }

    #[test]
    fn diagnostic_battery_state_merges_answers_alongside_the_battery_fields() {
        let json = serde_json::json!({
            "goalAlignment": "Mide la brecha",
            "questions": [],
            "answers": {"0": "Se esparce sola"}
        });
        let parsed: DiagnosticBatteryState = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.battery.goal_alignment, "Mide la brecha");
        assert_eq!(parsed.answers.get("0"), Some(&"Se esparce sola".to_string()));
    }

    #[test]
    fn interactive_prediction_gate_parses_camel_case_wire_shape() {
        let json = serde_json::json!({
            "blockType": "interactive_prediction_gate",
            "question": "¿Qué crees que imprime este código?",
            "options": ["1", "2", "error"],
            "conceptualFeedbackMap": {"1": "Casi, revisa el índice", "2": "Correcto", "error": "No lanza error aquí"},
            "correctOption": "2"
        });
        let parsed: GeneratedSectionBlock = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.block_type(), DynamicBlockType::InteractivePredictionGate);
    }

    #[test]
    fn gate_submission_parses_by_block_type_tag() {
        let json = serde_json::json!({"blockType": "interactive_prediction_gate", "selectedOption": "2"});
        let parsed: GateSubmission = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.block_type(), DynamicBlockType::InteractivePredictionGate);

        let json = serde_json::json!({"blockType": "hands_on_mission", "submissionText": "mi solución"});
        let parsed: GateSubmission = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.block_type(), DynamicBlockType::HandsOnMission);
    }

    #[test]
    fn block_status_round_trips_through_its_wire_string() {
        for s in [BlockStatus::Ready, BlockStatus::Passed, BlockStatus::Failed, BlockStatus::Escalated] {
            assert_eq!(BlockStatus::parse(s.as_str()), Some(s));
        }
        assert!(BlockStatus::Ready.unlocked(DynamicBlockType::AnchoredMicroTheory), "content blocks unlock immediately");
        assert!(!BlockStatus::Ready.unlocked(DynamicBlockType::InteractivePredictionGate), "an unanswered gate stays locked");
        assert!(BlockStatus::Passed.unlocked(DynamicBlockType::InteractivePredictionGate));
        assert!(BlockStatus::Escalated.unlocked(DynamicBlockType::InteractivePredictionGate));
        assert!(!BlockStatus::Failed.unlocked(DynamicBlockType::InteractivePredictionGate));
    }

    #[test]
    fn declarative_svg_and_conceptual_matrix_parse() {
        let svg = serde_json::json!({
            "renderEngine": "declarative_svg",
            "elements": [{"tag": "rect", "props": {"x": 0, "y": 0, "width": 10, "height": 10}, "label": "caja"}],
            "caption": "Diagrama de cajas"
        });
        let parsed: StaticVisualSpec = serde_json::from_value(svg).expect("parses");
        assert!(matches!(parsed, StaticVisualSpec::DeclarativeSvg { .. }));

        let matrix = serde_json::json!({
            "renderEngine": "conceptual_matrix",
            "headers": ["Enfoque", "Ventaja"],
            "rows": [["A", "Rápido"], ["B", "Simple"]],
            "contrastFocus": "Velocidad vs. simplicidad"
        });
        let parsed: StaticVisualSpec = serde_json::from_value(matrix).expect("parses");
        assert!(matches!(parsed, StaticVisualSpec::ConceptualMatrix { .. }));
    }

    #[test]
    fn generated_dynamic_notebook_parses_camel_case_wire_shape() {
        let json = serde_json::json!({
            "topicTitle": "Depurando un bucle infinito",
            "pedagogicalRationale": "El tema es puramente procedimental, así que se prioriza auditar el error real antes de practicar",
            "sections": [
                {"blockType": "anchored_micro_theory", "title": "t", "intuitiveHook": "h", "systemRule": "r", "frequentError": "err"},
                {"blockType": "heuristic_error_audit", "instruction": "i",
                 "flawedRepresentation": {"context": "c", "buggySnippetOrDiagram": "s", "errorType": "syntax"},
                 "guidingQuestions": ["q1"], "modelSolution": "sol"},
                {"blockType": "hands_on_mission", "challengeStatement": "c", "expectedMilestoneArtifact": "a",
                 "constraints": ["sin librerías externas"], "scaffoldingHints": ["h1"], "evaluationRubricSummary": ["r1"]}
            ]
        });
        let parsed: GeneratedDynamicNotebook = serde_json::from_value(json).expect("parses");
        assert_eq!(parsed.sections.len(), 3);
        assert_eq!(parsed.topic_title, "Depurando un bucle infinito");
    }

    /// Regression guard against exactly the class of bug that produced
    /// "Este bloque no se pudo mostrar (formato inesperado)" in production:
    /// a Rust field renamed/added without the frontend's
    /// `DynamicSectionBlockSchema` (in `src/lib/schemas.ts`) updated to
    /// match, so a freshly-generated (not stale) notebook still fails to
    /// parse client-side. Every variant's SERIALIZED key set is checked
    /// here against the exact field list the Zod schema requires — if this
    /// test ever needs updating, the frontend schema needs the SAME update
    /// in the same change.
    #[test]
    fn every_block_variant_serializes_to_exactly_the_frontend_contract_fields() {
        use std::collections::HashSet;

        fn keys(block: &GeneratedSectionBlock) -> HashSet<String> {
            serde_json::to_value(block).expect("serializes").as_object().expect("is an object").keys().cloned().collect()
        }
        fn expect(block: &GeneratedSectionBlock, fields: &[&str]) {
            let expected: HashSet<String> = fields.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                keys(block),
                expected,
                "{:?} drifted from the frontend DynamicSectionBlockSchema contract",
                block.block_type()
            );
        }

        expect(
            &GeneratedSectionBlock::SpacedInterleavedRetrieval {
                items: vec![RetrievalPrompt {
                    concept_label: "Ósmosis".to_string(),
                    prompt: "¿Hacia dónde se mueve el agua?".to_string(),
                    expected_answer: "Hacia la mayor concentración de soluto".to_string(),
                }],
            },
            &["blockType", "items"],
        );

        expect(
            &GeneratedSectionBlock::AnchoredMicroTheory {
                title: "t".to_string(),
                intuitive_hook: "h".to_string(),
                analogy_boundary: None,
                system_rule: "r".to_string(),
                frequent_error: "e".to_string(),
            },
            &["blockType", "title", "intuitiveHook", "systemRule", "frequentError"],
        );

        expect(
            &GeneratedSectionBlock::DeclarativeVisualDiagram {
                title: "t".to_string(),
                visual_aid: StaticVisualSpec::Mermaid {
                    chart_type: MermaidChartType::Flowchart,
                    code: "graph TD; A-->B;".to_string(),
                    caption: "c".to_string(),
                },
                guided_walkthrough: vec![],
            },
            &["blockType", "title", "visualAid", "guidedWalkthrough"],
        );

        expect(
            &GeneratedSectionBlock::BranchingScenarioChallenge {
                scenario: "s".to_string(),
                decision_point: "d".to_string(),
                branches: vec![ScenarioBranch { choice: "a".to_string(), consequence: "b".to_string(), is_optimal: true }],
                visual_aid: None,
            },
            &["blockType", "scenario", "decisionPoint", "branches"],
        );

        expect(
            &GeneratedSectionBlock::HeuristicErrorAudit {
                instruction: "i".to_string(),
                flawed_representation: FlawedRepresentation {
                    context: "c".to_string(),
                    buggy_snippet_or_diagram: "s".to_string(),
                    error_type: FlawErrorType::Syntax,
                },
                guiding_questions: vec![],
                model_solution: "sol".to_string(),
                visual_aid: None,
            },
            &["blockType", "instruction", "flawedRepresentation", "guidingQuestions", "modelSolution"],
        );

        expect(
            &GeneratedSectionBlock::InteractivePredictionGate {
                question: "q".to_string(),
                options: vec![],
                conceptual_feedback_map: Default::default(),
                correct_option: "a".to_string(),
                visual_aid: None,
            },
            &["blockType", "question", "options", "conceptualFeedbackMap", "correctOption"],
        );

        expect(
            &GeneratedSectionBlock::HandsOnMission {
                challenge_statement: "c".to_string(),
                expected_milestone_artifact: "a".to_string(),
                constraints: vec![],
                scaffolding_hints: vec![],
                evaluation_rubric_summary: vec![],
                is_transfer: false,
                visual_aid: None,
            },
            &["blockType", "challengeStatement", "expectedMilestoneArtifact", "constraints", "scaffoldingHints", "evaluationRubricSummary"],
        );

        expect(
            &GeneratedSectionBlock::MetacognitiveClosure {
                synthesis_task: "t".to_string(),
                prediction_comparison: PredictionComparison {
                    initial_prediction: "ip".to_string(),
                    final_result: "fr".to_string(),
                    contrast_narrative: "cn".to_string(),
                },
                self_evaluation_checklist: vec![],
            },
            &["blockType", "synthesisTask", "predictionComparison", "selfEvaluationChecklist"],
        );
    }

    /// `visual_aid` on the 4 gate-eligible block types must serialize under
    /// the same camelCase `visualAid` key as `declarative_visual_diagram`'s
    /// (and be OMITTED, not `null`, when absent — checked above) so the
    /// frontend's optional `visualAid` field lines up either way.
    #[test]
    fn optional_visual_aid_serializes_under_the_same_camel_case_key_when_present() {
        let visual = StaticVisualSpec::ConceptualMatrix {
            headers: vec!["a".to_string()],
            rows: vec![vec!["b".to_string()]],
            contrast_focus: "f".to_string(),
        };
        let block = GeneratedSectionBlock::InteractivePredictionGate {
            question: "q".to_string(),
            options: vec!["a".to_string()],
            conceptual_feedback_map: Default::default(),
            correct_option: "a".to_string(),
            visual_aid: Some(visual),
        };
        let value = serde_json::to_value(&block).expect("serializes");
        assert!(value.get("visualAid").is_some(), "{value:?}");
    }

    /// Same backward-compat convention, mirrored for `analogyBoundary`:
    /// omitted (not `null`) when absent — so blocks persisted before this
    /// field existed still deserialize — and present under the camelCase
    /// key when the model does supply it.
    #[test]
    fn optional_analogy_boundary_is_omitted_when_absent_and_present_under_camel_case_key_when_set() {
        let without = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "t".to_string(),
            intuitive_hook: "h".to_string(),
            analogy_boundary: None,
            system_rule: "r".to_string(),
            frequent_error: "e".to_string(),
        };
        let value = serde_json::to_value(&without).expect("serializes");
        assert!(value.get("analogyBoundary").is_none(), "{value:?}");

        let with = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "t".to_string(),
            intuitive_hook: "h".to_string(),
            analogy_boundary: Some("No explica qué pasa con escritura concurrente".to_string()),
            system_rule: "r".to_string(),
            frequent_error: "e".to_string(),
        };
        let value = serde_json::to_value(&with).expect("serializes");
        assert!(value.get("analogyBoundary").is_some(), "{value:?}");
    }

    /// A block SERIALIZED before `analogyBoundary` existed (no key at all)
    /// must still DESERIALIZE cleanly — this is the exact bug class the
    /// `.nullish()`/`Option<T>` + `#[serde(default)]` convention exists to
    /// prevent (see `analogy_boundary`'s doc comment).
    #[test]
    fn anchored_micro_theory_deserializes_when_analogy_boundary_key_is_entirely_absent() {
        let json = serde_json::json!({
            "blockType": "anchored_micro_theory",
            "title": "t",
            "intuitiveHook": "h",
            "systemRule": "r",
            "frequentError": "e"
        });
        let block: GeneratedSectionBlock = serde_json::from_value(json).expect("parses pre-existing stored blocks");
        match block {
            GeneratedSectionBlock::AnchoredMicroTheory { analogy_boundary, .. } => assert_eq!(analogy_boundary, None),
            other => panic!("expected AnchoredMicroTheory, got {other:?}"),
        }
    }

    fn note_doc(current_block_index: u32) -> NotebookDocument {
        NotebookDocument {
            id: "doc-1".into(),
            class_id: "class-1".into(),
            title: "Clase".into(),
            status: NotebookStatus::Ready,
            updated_at_ms: 0,
            pedagogical_rationale: None,
            current_block_index,
            initial_prediction: None,
        }
    }

    fn note_block(block_type: DynamicBlockType, status: BlockStatus, order_index: u32) -> NotebookBlock {
        NotebookBlock {
            id: format!("b-{order_index}"),
            document_id: "doc-1".into(),
            block_type,
            content_json: serde_json::json!({}),
            order_index,
            status,
            attempt_count: 0,
            last_feedback: None,
        }
    }

    fn closure_with_reflection(reflection: &str, order_index: u32) -> NotebookBlock {
        let mut block = note_block(DynamicBlockType::MetacognitiveClosure, BlockStatus::Passed, order_index);
        block.content_json = serde_json::json!({ "studentReflection": reflection });
        block
    }

    #[test]
    fn a_class_completes_when_every_gate_is_resolved_and_the_closure_feedback_passed() {
        let blocks = vec![
            note_block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, 0),
            note_block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Escalated, 1),
            note_block(DynamicBlockType::AnchoredMicroTheory, BlockStatus::Ready, 2),
            closure_with_reflection("Puedo explicar la entrada y la salida", 3),
        ];
        assert!(class_is_complete(&note_doc(4), &blocks));
    }

    #[test]
    fn an_unresolved_gate_or_a_missing_reflection_keeps_the_class_incomplete() {
        let cases: Vec<(NotebookDocument, Vec<NotebookBlock>, &str)> = vec![
            (
                note_doc(2),
                vec![
                    note_block(DynamicBlockType::InteractivePredictionGate, BlockStatus::Ready, 0),
                    closure_with_reflection("Ya sé la idea", 1),
                ],
                "a gate still awaiting submission",
            ),
            (
                note_doc(2),
                vec![
                    note_block(DynamicBlockType::HeuristicErrorAudit, BlockStatus::Failed, 0),
                    closure_with_reflection("Ya sé la idea", 1),
                ],
                "a failed (not yet escalated) gate",
            ),
            (
                note_doc(2),
                vec![
                    note_block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, 0),
                    closure_with_reflection("   ", 1),
                ],
                "a blank reflection",
            ),
            (
                note_doc(2),
                vec![
                    note_block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, 0),
                    note_block(DynamicBlockType::MetacognitiveClosure, BlockStatus::Ready, 1),
                ],
                "a closure with no reflection at all",
            ),
            (
                note_doc(2),
                {
                    let mut pending =
                        note_block(DynamicBlockType::MetacognitiveClosure, BlockStatus::Failed, 1);
                    pending.content_json = serde_json::json!({ "studentReflection": "Ya sé la idea" });
                    vec![note_block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, 0), pending]
                },
                "a reflection whose closure feedback never passed",
            ),
            (
                note_doc(1),
                vec![note_block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, 0)],
                "no closure at all",
            ),
            (
                note_doc(1),
                vec![
                    note_block(DynamicBlockType::HandsOnMission, BlockStatus::Passed, 0),
                    closure_with_reflection("Ya sé la idea", 1),
                ],
                "a closure that was never revealed (cursor still on it)",
            ),
        ];
        for (doc, blocks, why) in cases {
            assert!(!class_is_complete(&doc, &blocks), "{why}");
        }
    }
