use super::harness::*;
use super::*;

    #[tokio::test]
    async fn ungrounded_assessment_is_rejected_then_a_corrected_one_closes_on_retry() {
        let battery = full_battery();
        let mut bad = full_assessment("", 4, 3.0, EntryLevel::TheoreticalFoundations); // empty topic
        bad.topic = String::new();
        let bad_step = || ScriptedStep { text: "..".to_string(), assessment: Some(bad.clone()), ..Default::default() };
        let good_step = ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            assessment: Some(full_assessment("Ciclo de Krebs", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        let propose_step = full_propose_step("¿Te parece bien así?", 4, 3.0, "Ciclo de Krebs");
        let confirm_step = full_confirm_step("Listo, guardado.");
        // 2 CONSECUTIVE bad attempts exhaust the in-turn grounding-retry
        // budget (MAX_GROUNDING_RETRIES) without self-healing — proving a
        // genuinely persistent failure still needs the student's real next
        // message, unlike the single-bad-attempt case covered by
        // `an_ungrounded_diagnostic_battery_self_corrects_within_the_same_turn`.
        let harness =
            service_with_runner(Arc::new(ScriptedRunner::new(vec![bad_step(), bad_step(), good_step, propose_step, confirm_step])));

        let r0 = harness.service.start_session(None).await.expect("start ok");
        assert_eq!(r0.session.status, SessionStatus::Active);
        let sid = r0.session.session_id.clone();

        let r1 = harness.service.send_message(None, &sid, "Quiero algo").await.expect("turn ok");
        assert_eq!(r1.session.status, SessionStatus::Active, "bad grounding must not save the profile, even after the internal retry");
        assert!(r1.session.learner_profile_card.is_none());
        assert!(!r1.session.last_rejection_reasons.is_empty());
        let sid = r1.session.session_id.clone();

        let r2 = harness.service.send_message(None, &sid, "Ciclo de Krebs, 4 semanas, 3h").await.expect("turn ok");
        assert_eq!(r2.session.status, SessionStatus::Active, "assessment corrected, now waiting on the battery");
        assert!(r2.session.pending_diagnostic_battery.is_some());

        let r3 = answer_all_questions(&harness, &sid, &battery).await;
        assert_eq!(r3.session.status, SessionStatus::Active, "answering the battery only proposes — confirmation still pending");
        assert!(r3.session.proposed_plan.is_some());

        let r4 = harness.service.send_message(None, &sid, "Sí, confirmo").await.expect("confirm ok");
        assert_eq!(r4.session.status, SessionStatus::Sealed, "corrected assessment + answered battery + explicit confirmation seal");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn an_ungrounded_diagnostic_battery_self_corrects_within_the_same_turn() {
        // The exact bug this fixed: a rejected present_diagnostic_battery
        // used to leave the student staring at "Antes de armar tu plan..."
        // with no battery ever rendering — nothing told them anything had
        // gone wrong — until they happened to send ANOTHER message, which
        // silently carried the fix. Now the correction happens internally,
        // inside this ONE send_message call.
        let battery = full_battery();
        let bad_battery_step = ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            assessment: Some(full_assessment("Ciclo de Krebs", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(DiagnosticBattery { goal_alignment: "x".to_string(), questions: vec![] }), // needs 3-4
            ..Default::default()
        };
        let good_battery_step = ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            diagnostic_battery: Some(battery),
            ..Default::default()
        };
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![bad_battery_step, good_battery_step])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();

        let result = harness.service.send_message(None, &sid, "Ciclo de Krebs, 4 semanas, 3h").await.expect("turn ok");

        assert!(result.session.learner_profile_card.is_some(), "assessment saved on the first attempt");
        assert!(
            result.session.pending_diagnostic_battery.is_some(),
            "the corrected battery must render in THIS same turn, not the next one"
        );
        assert_eq!(result.session.turn_count_in_phase, 0, "reaching a stable checkpoint (even via internal retry) spends no gather turn");
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn a_monolithic_micromodule_over_5_hours_is_rejected() {
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 1, 14.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        // The exact anti-pattern the spec calls out: 14 hours crammed into
        // one undifferentiated block instead of 2 homogeneous sessions of
        // <=4h each.
        let monolithic_syllabus = RoadmapSyllabusPackage {
            course_title: "Tema".to_string(),
            total_weeks: 1,
            pace_hours_per_week: 14.0,
            milestones: vec![Milestone {
                week: 1,
                title: "Semana 1".to_string(),
                deliverable: "Entregable de la semana".to_string(),
                weekly_goal: Some("Avanzar en el tema".to_string()),
                micromodules: vec![Micromodule {
                    label: "Sesión única".to_string(),
                    hours: 14.0,
                    focus: Some("Todo el tema de una vez".to_string()),
                    deliverable: Deliverable {
                        artifact_type: DeliverableArtifactType::TestsPassing,
                        description: "Artefacto verificable".to_string(),
                    },
                    objective: Some("Explicar el flujo completo del ciclo".to_string()),
                    interactive_blocks: vec!["socratic_prediction".to_string(), "hands_on_mission".to_string(), "metacognitive_closure".to_string()],
                }],
            }],
            capstone_project: CapstoneProject {
                title: "Proyecto terminal".to_string(),
                description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
                verifiable_evidence: "Repositorio con la app corriendo + demo grabada".to_string(),
            },
        };
        let step2 = ScriptedStep {
            text: "..".to_string(),
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
                syllabus: monolithic_syllabus,
                closing_question: "¿Todo bien?".to_string(),
            }),
            ..Default::default()
        };
        // step2 queued twice: `finish_diagnostic_stage`'s in-turn grounding
        // retry consumes a 2nd scripted step when the monolithic block fails
        // again.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2.clone(), step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        let result = answer_all_questions(&harness, &sid, &battery).await;

        assert_eq!(result.session.status, SessionStatus::Active, "a monolithic (>5h) block must be rejected");
        assert!(result.session.proposed_plan.is_none(), "no proposal stored while grounding fails");
        assert!(
            result.session.last_rejection_reasons.iter().any(|r| r.contains("anti-monolito")),
            "rejection must name the anti-monolith rule: {:?}",
            result.session.last_rejection_reasons
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// Every module must say WHAT the student will be able to DO when it's
    /// done — the line the plan and the class header both display. A proposal
    /// that omits it (or words it as a banned vague phrase) is rejected by the
    /// grounding pass, which is what feeds the correction back to the model.
    #[tokio::test]
    async fn a_micromodule_without_an_objective_is_rejected() {
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 1, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        let module = |label: &str, hours: f32, objective: Option<&str>| Micromodule {
            label: label.to_string(),
            hours,
            focus: Some("Conceptos centrales de la sesión".to_string()),
            deliverable: Deliverable {
                artifact_type: DeliverableArtifactType::TestsPassing,
                description: "Artefacto verificable".to_string(),
            },
            objective: objective.map(str::to_string),
            interactive_blocks: vec!["socratic_prediction".to_string(), "hands_on_mission".to_string(), "metacognitive_closure".to_string()],
        };
        // Well-formed EXCEPT for the missing objective: 2 homogeneous sessions
        // summing to paceHoursPerWeek, none over the 4h anti-monolith cap, so
        // the only violation the grounding pass can report is the objective
        // itself.
        let objectiveless_syllabus = RoadmapSyllabusPackage {
            course_title: "Tema".to_string(),
            total_weeks: 1,
            pace_hours_per_week: 3.0,
            milestones: vec![Milestone {
                week: 1,
                title: "Semana 1".to_string(),
                deliverable: "Entregable de la semana".to_string(),
                weekly_goal: Some("Avanzar en el tema".to_string()),
                micromodules: vec![
                    module("Sesión 1", 1.5, None),
                    module("Sesión 2", 1.5, Some("comprender la teoría")), // vague = still rejected
                ],
            }],
            capstone_project: CapstoneProject {
                title: "Proyecto terminal".to_string(),
                description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
                verifiable_evidence: "Repositorio con la app corriendo + demo grabada".to_string(),
            },
        };
        let step2 = ScriptedStep {
            text: "..".to_string(),
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
                syllabus: objectiveless_syllabus,
                closing_question: "¿Todo bien?".to_string(),
            }),
            ..Default::default()
        };
        // Queued twice so the in-turn grounding retry can't self-heal it —
        // the point is that the rejection names the missing objective.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2.clone(), step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        let result = answer_all_questions(&harness, &sid, &battery).await;

        assert_eq!(result.session.status, SessionStatus::Active, "a proposal missing objectives must be rejected");
        assert!(result.session.proposed_plan.is_none(), "no proposal stored while grounding fails");
        assert!(
            result.session.last_rejection_reasons.iter().any(|r| r.contains("objective")),
            "rejection must name the missing objective: {:?}",
            result.session.last_rejection_reasons
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// The typed `{artifactType, description}` deliverable (gaps #4/#5 of
    /// the pedagogical audit) closes the "vague but not on the denylist"
    /// hole a pure phrase-denylist left open: `artifactType: "other"` is the
    /// one escape hatch that isn't already self-describing, so it alone is
    /// held to a stricter minimum description length. A proposal that uses
    /// "other" with a description too short to actually describe anything
    /// is rejected, even though the text itself isn't on the vague-phrase
    /// denylist.
    #[tokio::test]
    async fn a_micromodule_deliverable_with_other_type_and_a_too_short_description_is_rejected() {
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 1, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        // Well-formed syllabus EXCEPT the first micromodule's deliverable:
        // not a denylisted vague phrase, but `artifactType: other` with a
        // description too short to actually describe an artifact.
        let mut short_other_syllabus = full_syllabus(1, 3.0, "Tema");
        short_other_syllabus.milestones[0].micromodules[0].deliverable = Deliverable {
            artifact_type: DeliverableArtifactType::Other,
            description: "Algo distinto".to_string(),
        };
        let step2 = ScriptedStep {
            text: "..".to_string(),
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
                syllabus: short_other_syllabus,
                closing_question: "¿Todo bien?".to_string(),
            }),
            ..Default::default()
        };
        // Queued twice so the in-turn grounding retry can't self-heal it —
        // the point is that the rejection names the too-short "other"
        // deliverable.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2.clone(), step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        let result = answer_all_questions(&harness, &sid, &battery).await;

        assert_eq!(result.session.status, SessionStatus::Active, "a too-short \"other\" deliverable must be rejected");
        assert!(result.session.proposed_plan.is_none(), "no proposal stored while grounding fails");
        assert!(
            result.session.last_rejection_reasons.iter().any(|r| r.contains("artifactType \"other\"")),
            "rejection must name the too-short other deliverable: {:?}",
            result.session.last_rejection_reasons
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    /// The terminal capstone project (gap #2 of the pedagogical audit) must
    /// carry an explicit, non-vague `description` and `verifiableEvidence` —
    /// distinct from the last milestone's own `deliverable`. A proposal that
    /// leaves either blank is rejected by the grounding pass, same as any
    /// other vague/empty content check.
    #[tokio::test]
    async fn a_capstone_project_missing_verifiable_evidence_is_rejected() {
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 1, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        // Well-formed syllabus EXCEPT the capstone's verifiableEvidence, so
        // the only violation the grounding pass can report is the capstone
        // itself.
        let mut capstoneless_syllabus = full_syllabus(1, 3.0, "Tema");
        capstoneless_syllabus.capstone_project = CapstoneProject {
            title: "Proyecto terminal".to_string(),
            description: "Integrar lo aprendido en un escenario real de transferencia".to_string(),
            verifiable_evidence: String::new(),
        };
        let step2 = ScriptedStep {
            text: "..".to_string(),
            propose_syllabus_plan: Some(ProposeSyllabusPlanArgs {
                core_focus: "Lo esencial".to_string(),
                identified_needs: vec!["Entender el flujo".to_string()],
                learning_strategy: "Guiado".to_string(),
                syllabus: capstoneless_syllabus,
                closing_question: "¿Todo bien?".to_string(),
            }),
            ..Default::default()
        };
        // Queued twice so the in-turn grounding retry can't self-heal it —
        // the point is that the rejection names the missing capstone evidence.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2.clone(), step2])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        let result = answer_all_questions(&harness, &sid, &battery).await;

        assert_eq!(result.session.status, SessionStatus::Active, "a proposal missing capstone evidence must be rejected");
        assert!(result.session.proposed_plan.is_none(), "no proposal stored while grounding fails");
        assert!(
            result.session.last_rejection_reasons.iter().any(|r| r.contains("capstoneProject.verifiableEvidence")),
            "rejection must name the missing capstone evidence: {:?}",
            result.session.last_rejection_reasons
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn a_length_biased_battery_question_is_rejected() {
        // The exact anti-pattern the spec calls out: the correct option is
        // longer AND embeds its own justification, letting a student answer
        // by elimination instead of by their actual mental model.
        let biased_battery = DiagnosticBattery {
            goal_alignment: "Mide la brecha".to_string(),
            questions: vec![
                DiagnosticQuestion {
                    dimension: DiagnosticDimension::Intuition,
                    prompt: "p1".to_string(),
                    options: vec![
                        "Sí".to_string(),
                        "No, porque el soluto siempre se difunde en contra del gradiente hasta igualarse".to_string(),
                    ],
                    correct_option: "Sí".to_string(),
                    diagnostic_insight: "insight".to_string(),
                },
                DiagnosticQuestion {
                    dimension: DiagnosticDimension::Mechanics,
                    prompt: "p2".to_string(),
                    options: vec!["A".to_string(), "B".to_string()],
                    correct_option: "A".to_string(),
                    diagnostic_insight: "insight".to_string(),
                },
                DiagnosticQuestion {
                    dimension: DiagnosticDimension::CriticalCase,
                    prompt: "p3".to_string(),
                    options: vec!["A".to_string(), "B".to_string()],
                    correct_option: "A".to_string(),
                    diagnostic_insight: "insight".to_string(),
                },
            ],
        };
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(biased_battery),
            ..Default::default()
        };
        // Queued twice: the in-turn grounding retry consumes a 2nd scripted
        // step when the biased battery fails again.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1.clone(), step1])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        let result = harness.service.send_message(None, &sid, "Hazlo").await.expect("turn ok");

        assert!(result.session.pending_diagnostic_battery.is_none(), "a biased battery must never reach the student");
        assert!(
            result.session.last_rejection_reasons.iter().any(|r| r.contains("sesgo de longitud") || r.contains("justificación")),
            "rejection must name the psychometric violation: {:?}",
            result.session.last_rejection_reasons
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

    #[tokio::test]
    async fn present_diagnostic_battery_resent_after_already_answered_is_rejected() {
        // The exact bug this fixed: after the battery is done and a plan is
        // already proposed, an ambiguous student reply ("No veo nada") used
        // to sometimes confuse the model into re-presenting the battery —
        // silently absorbed as a harmless no-op, leaving the misleading
        // "aquí está tu batería" text on screen forever with nothing behind
        // it (Gate 2 was already over, so the battery card never re-renders).
        let battery = full_battery();
        let step1 = ScriptedStep {
            text: "..".to_string(),
            assessment: Some(full_assessment("Tema", 4, 3.0, EntryLevel::TheoreticalFoundations)),
            diagnostic_battery: Some(battery.clone()),
            ..Default::default()
        };
        let step2 = full_propose_step("¿Te parece bien así?", 4, 3.0, "Tema");
        let resend_step = || ScriptedStep {
            text: "Antes de armar tu plan, quiero ver qué tanto conectas ya con esto:".to_string(),
            diagnostic_battery: Some(full_battery()),
            ..Default::default()
        };
        // resend_step queued twice: the in-turn grounding retry consumes a
        // 2nd scripted step when the confused resend is rejected again.
        let harness = service_with_runner(Arc::new(ScriptedRunner::new(vec![step1, step2, resend_step(), resend_step()])));
        let r0 = harness.service.start_session(None).await.expect("start ok");
        let sid = r0.session.session_id.clone();
        harness.service.send_message(None, &sid, "Tema, 4 semanas, 3h").await.expect("turn ok");
        let proposed = answer_all_questions(&harness, &sid, &battery).await;
        assert!(proposed.session.proposed_plan.is_some());

        let result = harness.service.send_message(None, &sid, "No veo nada").await.expect("turn ok");

        assert!(result.session.proposed_plan.is_some(), "the existing proposal must survive the confused resend");
        assert!(
            result.session.last_rejection_reasons.iter().any(|r| r.contains("ya fue respondida")),
            "rejection must name the resend-after-done rule: {:?}",
            result.session.last_rejection_reasons
        );
        assert_eq!(
            result.message,
            "Casi lo tengo — dame un momento más para ajustarlo. Escríbeme de nuevo (puede ser solo \"sigue\") para continuar.",
            "the misleading battery-intro text must never reach the student once grounding keeps failing"
        );
        let _ = std::fs::remove_dir_all(&harness.dir);
    }

