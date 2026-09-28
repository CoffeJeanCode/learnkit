use crate::domain::notebook::{DynamicBlockType, GeneratedSectionBlock, MermaidChartType, StaticVisualSpec, SvgTag};

const OPTION_LENGTH_TOLERANCE: f64 = 0.20;

const JUSTIFICATION_CONNECTORS: [&str; 6] = ["porque", "ya que", "debido a", "puesto que", "lo cual significa", "es decir que"];

const META_TALK_MARKERS: [&str; 5] = [
    "calibrando",
    "como modelo de lenguaje",
    "como inteligencia artificial",
    "según mis directrices",
    "generando el bloque",
];

pub fn needs_llm_critic(block_type: DynamicBlockType) -> bool {
    matches!(
        block_type,
        DynamicBlockType::AnchoredMicroTheory
            | DynamicBlockType::InteractivePredictionGate
            | DynamicBlockType::BranchingScenarioChallenge
    )
}

pub fn block_guardrail_violations(block: &GeneratedSectionBlock) -> Vec<String> {
    let mut v = Vec::new();
    match block {
        GeneratedSectionBlock::SpacedInterleavedRetrieval { items } => {
            for (i, item) in items.iter().enumerate() {
                scan_text(&format!("items[{i}].conceptLabel"), &item.concept_label, &mut v);
                scan_text(&format!("items[{i}].prompt"), &item.prompt, &mut v);
                scan_text(&format!("items[{i}].expectedAnswer"), &item.expected_answer, &mut v);
            }
        }
        GeneratedSectionBlock::AnchoredMicroTheory { title, intuitive_hook, system_rule, frequent_error } => {
            scan_text("title", title, &mut v);
            scan_text("intuitiveHook", intuitive_hook, &mut v);
            scan_text("systemRule", system_rule, &mut v);
            scan_text("frequentError", frequent_error, &mut v);
        }
        GeneratedSectionBlock::DeclarativeVisualDiagram { title, visual_aid, guided_walkthrough } => {
            scan_text("title", title, &mut v);
            for step in guided_walkthrough {
                scan_text("guidedWalkthrough.targetVisualElement", &step.target_visual_element, &mut v);
                scan_text("guidedWalkthrough.pedagogicalInsight", &step.pedagogical_insight, &mut v);
            }
            v.extend(visual_violations(visual_aid, "visualAid"));
        }
        GeneratedSectionBlock::BranchingScenarioChallenge { scenario, decision_point, branches, visual_aid } => {
            scan_text("scenario", scenario, &mut v);
            scan_text("decisionPoint", decision_point, &mut v);
            for (i, branch) in branches.iter().enumerate() {
                scan_text(&format!("branches[{i}].choice"), &branch.choice, &mut v);
                scan_text(&format!("branches[{i}].consequence"), &branch.consequence, &mut v);
            }
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(visual, "visualAid"));
            }
        }
        GeneratedSectionBlock::HeuristicErrorAudit {
            instruction,
            flawed_representation,
            guiding_questions,
            model_solution,
            visual_aid,
        } => {
            scan_text("instruction", instruction, &mut v);
            scan_text("flawedRepresentation.context", &flawed_representation.context, &mut v);
            scan_text(
                "flawedRepresentation.buggySnippetOrDiagram",
                &flawed_representation.buggy_snippet_or_diagram,
                &mut v,
            );
            for (i, q) in guiding_questions.iter().enumerate() {
                scan_text(&format!("guidingQuestions[{i}]"), q, &mut v);
            }
            scan_text("modelSolution", model_solution, &mut v);
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(visual, "visualAid"));
            }
        }
        GeneratedSectionBlock::InteractivePredictionGate {
            question,
            options,
            conceptual_feedback_map,
            correct_option: _,
            visual_aid,
        } => {
            scan_text("question", question, &mut v);
            v.extend(option_set_violations("options", options));
            for (option, feedback) in conceptual_feedback_map {
                scan_text(&format!("conceptualFeedbackMap[\"{option}\"]"), feedback, &mut v);
            }
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(visual, "visualAid"));
            }
        }
        GeneratedSectionBlock::HandsOnMission {
            challenge_statement,
            expected_milestone_artifact,
            constraints,
            scaffolding_hints,
            evaluation_rubric_summary,
            visual_aid,
        } => {
            scan_text("challengeStatement", challenge_statement, &mut v);
            scan_text("expectedMilestoneArtifact", expected_milestone_artifact, &mut v);
            scan_list("constraints", constraints, &mut v);
            scan_list("scaffoldingHints", scaffolding_hints, &mut v);
            scan_list("evaluationRubricSummary", evaluation_rubric_summary, &mut v);
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(visual, "visualAid"));
            }
        }
        GeneratedSectionBlock::MetacognitiveClosure { synthesis_task, prediction_comparison, self_evaluation_checklist } => {
            scan_text("synthesisTask", synthesis_task, &mut v);
            scan_text("predictionComparison.initialPrediction", &prediction_comparison.initial_prediction, &mut v);
            scan_text("predictionComparison.finalResult", &prediction_comparison.final_result, &mut v);
            scan_text("predictionComparison.contrastNarrative", &prediction_comparison.contrast_narrative, &mut v);
            scan_list("selfEvaluationChecklist", self_evaluation_checklist, &mut v);
        }
    }
    v
}

fn scan_list(field: &str, items: &[String], out: &mut Vec<String>) {
    for (i, item) in items.iter().enumerate() {
        scan_text(&format!("{field}[{i}]"), item, out);
    }
}

fn scan_text(field: &str, text: &str, out: &mut Vec<String>) {
    let lower = text.to_lowercase();
    for marker in META_TALK_MARKERS {
        if lower.contains(marker) {
            out.push(format!("bloque: {field} contiene lenguaje meta del sistema (\"{marker}\") — proibido dirigirse al estudiante como generador"));
        }
    }
}

pub fn option_set_violations(field: &str, options: &[String]) -> Vec<String> {
    let mut v = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for option in options {
        if !seen.insert(option.trim().to_lowercase()) {
            v.push(format!("bloque: {field} contiene opciones duplicadas (\"{option}\")"));
        }
        let lower = option.to_lowercase();
        for connector in JUSTIFICATION_CONNECTORS {
            if lower.contains(connector) {
                v.push(format!(
                    "bloque: {field} contiene una opción (\"{option}\") con su propia justificación (\"{connector}\") — \
                     la razón pertenece al feedback, nunca al texto de la opción"
                ));
                break;
            }
        }
    }
    if options.len() >= 2 {
        let lengths: Vec<usize> = options.iter().map(|o| o.chars().count()).collect();
        if let (Some(&max_len), Some(&min_len)) = (lengths.iter().max(), lengths.iter().min()) {
            if max_len > 0 && (max_len - min_len) as f64 / max_len as f64 > OPTION_LENGTH_TOLERANCE {
                v.push(format!(
                    "bloque: {field} tiene opciones con longitudes muy dispares (sesgo de longitud) — \
                     min {min_len} / max {max_len} caracteres, variación permitida < 20%"
                ));
            }
        }
    }
    v
}

fn visual_violations(visual: &StaticVisualSpec, field: &str) -> Vec<String> {
    let mut v = Vec::new();
    match visual {
        StaticVisualSpec::Mermaid { chart_type, code, .. } => v.extend(mermaid_violations(*chart_type, code, field)),
        StaticVisualSpec::DeclarativeSvg { view_box, elements, groups, .. } => {
            v.extend(svg_violations(view_box, elements, &format!("{field}.elements")));
            for group in groups {
                v.extend(svg_violations(view_box, &group.elements, &format!("{field}.groups[{}]", group.group_id)));
            }
        }
        StaticVisualSpec::ConceptualMatrix { contrast_focus, .. } => scan_text(&format!("{field}.contrastFocus"), contrast_focus, &mut v),
    }
    v
}

fn mermaid_violations(chart_type: MermaidChartType, code: &str, field: &str) -> Vec<String> {
    let mut v = Vec::new();
    let trimmed = code.trim();
    if trimmed.is_empty() {
        v.push(format!("bloque: {field} mermaid sin código"));
        return v;
    }
    let first_line = trimmed.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim().to_lowercase();
    let first_token = first_line.split_whitespace().next().unwrap_or_default();
    let expected: &[&str] = match chart_type {
        MermaidChartType::Flowchart => &["flowchart", "graph"],
        MermaidChartType::SequenceDiagram => &["sequencediagram"],
        MermaidChartType::ClassDiagram => &["classdiagram"],
        MermaidChartType::StateDiagram => &["statediagram", "statediagram-v2"],
        MermaidChartType::ErDiagram => &["erdiagram"],
    };
    if !expected.contains(&first_token) {
        v.push(format!(
            "bloque: {field} mermaid no abre con el tipo declarado ({:?} debería empezar con \"{}\") — empieza con \"{first_token}\"",
            chart_type,
            expected.join("\" | \"")
        ));
    }
    for (open, close, name) in [('[', ']', "corchetes"), ('(', ')', "paréntesis")] {
        let opens = trimmed.matches(open).count();
        let closes = trimmed.matches(close).count();
        if opens != closes {
            v.push(format!("bloque: {field} mermaid con {name} desbalanceados ({opens} vs {closes}) — sintaxis inválida"));
        }
    }
    if trimmed.matches('"').count() % 2 != 0 {
        v.push(format!("bloque: {field} mermaid con comillas sin cerrar — sintaxis inválida"));
    }
    v
}

fn svg_violations(view_box: &str, elements: &[crate::domain::notebook::SvgElement], field: &str) -> Vec<String> {
    let mut v = Vec::new();
    let tokens: Vec<&str> = view_box.split_whitespace().collect();
    let numeric = tokens.len() == 4 && tokens.iter().all(|t| t.parse::<f64>().is_ok());
    if !numeric {
        v.push(format!("bloque: {field} tiene viewBox inválido (\"{view_box}\") — se espera \"0 0 800 450\""));
    }
    for (i, element) in elements.iter().enumerate() {
        if element.tag == SvgTag::Text {
            let backed = i > 0 && elements[i - 1].tag == SvgTag::Rect;
            if !backed {
                v.push(format!(
                    "bloque: {field}: un text (índice {i}) no va precedido de su rect de fondo (pill) — \
                     ningún texto flota sobre el trazo"
                ));
            }
        }
        if let Some(props) = element.props.as_object() {
            for key in props.keys() {
                if key.contains('-') && !key.starts_with("data-") && !key.starts_with("aria-") {
                    v.push(format!(
                        "bloque: {field}: prop \"{key}\" está en kebab-case — usa camelCase de React (fillOpacity, strokeDasharray…)"
                    ));
                }
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::domain::notebook::{PredictionComparison, ScenarioBranch, SvgElement, SvgGroup, WalkthroughStep};

    fn valid_gate() -> GeneratedSectionBlock {
        GeneratedSectionBlock::InteractivePredictionGate {
            question: "¿Qué pasa primero?".to_string(),
            options: vec!["Sí".to_string(), "No".to_string()],
            conceptual_feedback_map: HashMap::from([
                ("Sí".to_string(), "Casi — revisa el gradiente".to_string()),
                ("No".to_string(), "Exacto: difusión neta hacia adentro".to_string()),
            ]),
            correct_option: "No".to_string(),
            visual_aid: None,
        }
    }

    fn valid_theory() -> GeneratedSectionBlock {
        GeneratedSectionBlock::AnchoredMicroTheory {
            title: "La gran imagen".to_string(),
            intuitive_hook: "Piensa en una fábrica circular".to_string(),
            system_rule: "Explicación breve del tema.".to_string(),
            frequent_error: "Confundir la entrada con la salida".to_string(),
        }
    }

    #[test]
    fn clean_blocks_pass() {
        assert!(block_guardrail_violations(&valid_gate()).is_empty());
        assert!(block_guardrail_violations(&valid_theory()).is_empty());
    }

    #[test]
    fn flags_length_biased_options() {
        let mut block = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { options, .. } = &mut block {
            options[1] = "No, porque el soluto se difunde en contra del gradiente hasta igualarse".to_string();
        }
        let v = block_guardrail_violations(&block);
        assert!(v.iter().any(|s| s.contains("sesgo de longitud")), "{v:?}");
    }

    #[test]
    fn flags_options_that_explain_themselves() {
        let mut block = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { options, .. } = &mut block {
            options[0] = "Sí, ya que el transporte es pasivo".to_string();
            options[1] = "No, difusión activa".to_string();
        }
        let v = block_guardrail_violations(&block);
        assert!(v.iter().any(|s| s.contains("justificación")), "{v:?}");
    }

    #[test]
    fn flags_duplicate_options() {
        let mut block = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { options, .. } = &mut block {
            options[1] = "sí".to_string();
        }
        let v = block_guardrail_violations(&block);
        assert!(v.iter().any(|s| s.contains("duplicadas")), "{v:?}");
    }

    #[test]
    fn flags_generation_meta_talk() {
        let block = GeneratedSectionBlock::AnchoredMicroTheory {
            title: "T".to_string(),
            intuitive_hook: "Calibrando el nivel del estudiante…".to_string(),
            system_rule: "R".to_string(),
            frequent_error: "E".to_string(),
        };
        let v = block_guardrail_violations(&block);
        assert!(v.iter().any(|s| s.contains("lenguaje meta")), "{v:?}");
    }

    #[test]
    fn flags_mermaid_that_does_not_open_with_its_declared_type() {
        let v = mermaid_violations(MermaidChartType::Flowchart, "sequenceDiagram\n  A->>B: hola", "visualAid");
        assert!(v.iter().any(|s| s.contains("no abre con el tipo declarado")), "{v:?}");

        let ok = mermaid_violations(MermaidChartType::Flowchart, "flowchart TD\n  A[Uno] --> B[Dos]", "visualAid");
        assert!(ok.is_empty(), "{ok:?}");
    }

    #[test]
    fn flags_unbalanced_mermaid() {
        let v = mermaid_violations(MermaidChartType::Flowchart, "flowchart TD\n  A[Uno --> B", "visualAid");
        assert!(v.iter().any(|s| s.contains("desbalanceados")), "{v:?}");
    }

    #[test]
    fn flags_svg_text_without_a_pill_and_kebab_case_props() {
        let elements = vec![
            SvgElement { tag: SvgTag::Text, props: serde_json::json!({"x": 10, "y": 20, "fill-opacity": 0.5}), label: None },
            SvgElement { tag: SvgTag::Circle, props: serde_json::json!({"cx": 20, "cy": 30, "r": 5}), label: None },
        ];
        let v = svg_violations("0 0 800 450", &elements, "visualAid.elements");
        assert!(v.iter().any(|s| s.contains("no va precedido de su rect")), "{v:?}");
        assert!(v.iter().any(|s| s.contains("kebab-case")), "{v:?}");
    }

    #[test]
    fn flags_svg_text_backed_by_a_rect() {
        let elements = vec![
            SvgElement { tag: SvgTag::Rect, props: serde_json::json!({"x": 8, "y": 16, "fillOpacity": 0.85}), label: None },
            SvgElement { tag: SvgTag::Text, props: serde_json::json!({"x": 12, "y": 30}), label: None },
        ];
        let v = svg_violations("0 0 800 450", &elements, "visualAid.elements");
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn flags_invalid_view_box() {
        let v = svg_violations("centered scene", &[], "visualAid.elements");
        assert!(v.iter().any(|s| s.contains("viewBox inválido")), "{v:?}");
    }

    #[test]
    fn only_semantic_rubric_types_require_the_llm_critic() {
        assert!(needs_llm_critic(DynamicBlockType::AnchoredMicroTheory));
        assert!(needs_llm_critic(DynamicBlockType::InteractivePredictionGate));
        assert!(needs_llm_critic(DynamicBlockType::BranchingScenarioChallenge));
        assert!(!needs_llm_critic(DynamicBlockType::DeclarativeVisualDiagram));
        assert!(!needs_llm_critic(DynamicBlockType::HeuristicErrorAudit));
        assert!(!needs_llm_critic(DynamicBlockType::HandsOnMission));
        assert!(!needs_llm_critic(DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn full_block_variants_pass_or_flag_as_expected() {
        let diagram = GeneratedSectionBlock::DeclarativeVisualDiagram {
            title: "Transporte".to_string(),
            visual_aid: StaticVisualSpec::DeclarativeSvg {
                view_box: "0 0 800 450".to_string(),
                elements: vec![
                    SvgElement { tag: SvgTag::Rect, props: serde_json::json!({"x": 0}), label: None },
                    SvgElement { tag: SvgTag::Text, props: serde_json::json!({"x": 4}), label: None },
                ],
                groups: vec![SvgGroup {
                    group_id: "zona".to_string(),
                    pedagogical_role: "zona".to_string(),
                    elements: vec![SvgElement { tag: SvgTag::Circle, props: serde_json::json!({"cx": 1}), label: None }],
                }],
                pedagogical_focus: None,
                caption: "El gradiente".to_string(),
            },
            guided_walkthrough: vec![WalkthroughStep {
                step_number: 1,
                target_visual_element: "la membrana".to_string(),
                pedagogical_insight: "Acá ocurre la selección".to_string(),
            }],
        };
        assert!(block_guardrail_violations(&diagram).is_empty());

        let scenario = GeneratedSectionBlock::BranchingScenarioChallenge {
            scenario: "El equipo no entiende la decisión.".to_string(),
            decision_point: "¿Comunicas ahora o esperas?".to_string(),
            branches: vec![
                ScenarioBranch { choice: "Comunicar ya".to_string(), consequence: "Se genera claridad".to_string(), is_optimal: true },
                ScenarioBranch { choice: "Esperar".to_string(), consequence: "Se pierde momentum".to_string(), is_optimal: false },
            ],
            visual_aid: None,
        };
        assert!(block_guardrail_violations(&scenario).is_empty());

        let closure = GeneratedSectionBlock::MetacognitiveClosure {
            synthesis_task: "Contrasta tu modelo inicial".to_string(),
            prediction_comparison: PredictionComparison {
                initial_prediction: "Que era pasivo".to_string(),
                final_result: "Que depende del gradiente y la energía".to_string(),
                contrast_narrative: "Tu predicción quedó corta frente al mecanismo real".to_string(),
            },
            self_evaluation_checklist: vec!["Puedo explicarlo".to_string()],
        };
        assert!(block_guardrail_violations(&closure).is_empty());
    }
}
