use crate::domain::notebook::{DynamicBlockType, GeneratedSectionBlock, MermaidChartType, StaticVisualSpec, SvgTag};

/// Options are length-biased only when BOTH hold: the relative gap
/// `(max - min) / max` exceeds this ratio AND the absolute gap exceeds
/// `OPTION_LENGTH_MIN_ABS_DIFF` characters. The absolute half keeps short
/// options (e.g. 36 vs 56 chars) from being flagged by the ratio alone.
const OPTION_LENGTH_TOLERANCE: f64 = 0.25;
const OPTION_LENGTH_MIN_ABS_DIFF: usize = 25;

/// Pure length-bias rule over option character counts.
pub fn options_are_length_biased(min_len: usize, max_len: usize) -> bool {
    let diff = max_len.saturating_sub(min_len);
    max_len > 0 && diff as f64 / max_len as f64 > OPTION_LENGTH_TOLERANCE && diff > OPTION_LENGTH_MIN_ABS_DIFF
}

/// Severity levels for adaptive slack in pedagogical assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum ValidationSeverity {
    Optimal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationFinding {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
    pub can_surgical_repair: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ValidationReport {
    pub findings: Vec<ValidationFinding>,
}

impl ValidationReport {
    pub fn is_acceptable(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == ValidationSeverity::Critical)
    }

    pub fn warnings(&self) -> Vec<&ValidationFinding> {
        self.findings.iter().filter(|f| f.severity == ValidationSeverity::Warning).collect()
    }

    pub fn criticals(&self) -> Vec<&ValidationFinding> {
        self.findings.iter().filter(|f| f.severity == ValidationSeverity::Critical).collect()
    }
}

pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Evaluates option set length using adaptive slack (Tier 1 Optimal, Tier 2 Warning/Slack, Tier 3 Critical).
pub fn evaluate_options_length_slack(options: &[String]) -> ValidationSeverity {
    if options.len() < 2 {
        return ValidationSeverity::Optimal;
    }
    let char_lens: Vec<usize> = options.iter().map(|o| o.chars().count()).collect();

    let min_chars = *char_lens.iter().min().unwrap_or(&0);
    let max_chars = *char_lens.iter().max().unwrap_or(&0);

    let char_diff = max_chars.saturating_sub(min_chars);
    let rel_char_diff = if max_chars > 0 { char_diff as f64 / max_chars as f64 } else { 0.0 };

    // Critical length bias: relative difference > 25% AND absolute difference > 25 characters
    if options_are_length_biased(min_chars, max_chars) {
        return ValidationSeverity::Critical;
    }

    // Tier 1: Optimal (variation <= 15% and absolute diff <= 15 chars)
    if rel_char_diff <= 0.15 && char_diff <= 15 {
        return ValidationSeverity::Optimal;
    }

    // Tier 2: Adaptive Slack (variation between 15% and 25%, or absolute gap <= 25 chars)
    ValidationSeverity::Warning
}

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
            // The figure IS the block: its coherence with the class is judgment.
            | DynamicBlockType::DeclarativeVisualDiagram
            | DynamicBlockType::InteractivePredictionGate
            | DynamicBlockType::BranchingScenarioChallenge
            // Exercises are where an unmodeled step hides: the critic checks
            // each one only demands what earlier blocks of the class modeled
            // (scaffolding monotonicity / "delta +1 variable").
            | DynamicBlockType::HeuristicErrorAudit
            | DynamicBlockType::HandsOnMission
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
        GeneratedSectionBlock::AnchoredMicroTheory { title, intuitive_hook, analogy_boundary, system_rule, frequent_error, visual_aid, .. } => {
            scan_text("title", title, &mut v);
            scan_text("intuitiveHook", intuitive_hook, &mut v);
            if let Some(boundary) = analogy_boundary {
                scan_text("analogyBoundary", boundary, &mut v);
            }
            scan_text("systemRule", system_rule, &mut v);
            scan_text("frequentError", frequent_error, &mut v);
            if let Some(visual) = visual_aid {
                v.extend(visual_violations(visual, "visualAid"));
            }
        }
        GeneratedSectionBlock::DeclarativeVisualDiagram { title, visual_aid, guided_walkthrough, .. } => {
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
            faulty_line_number: _,
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
            interaction_mode: _,
            shuffled_steps,
            correct_order: _,
            boundary_cases,
            visual_aid,
        } => {
            scan_text("question", question, &mut v);
            if let Some(steps) = shuffled_steps {
                scan_list("shuffledSteps", steps, &mut v);
            }
            for (i, case) in boundary_cases.iter().flatten().enumerate() {
                scan_text(&format!("boundaryCases[{i}].statement"), &case.statement, &mut v);
            }
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
            is_transfer,
            support_level: _,
            worked_template,
            interactive_steps,
            visual_aid,
        } => {
            if let Some(template) = worked_template {
                scan_text("workedTemplate", template, &mut v);
            }
            for (i, step) in interactive_steps.iter().flatten().enumerate() {
                scan_text(&format!("interactiveSteps[{i}].prompt"), &step.prompt, &mut v);
            }
            if *is_transfer && !scaffolding_hints.is_empty() {
                v.push("hands_on_mission de transferencia (isTransfer): scaffoldingHints debe ir vacío — la transferencia se evalúa sin pistas".to_string());
            }
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
            if options_are_length_biased(min_len, max_len) {
                v.push(format!(
                    "bloque: {field} tiene opciones con longitudes muy dispares (sesgo de longitud) — \
                     min {min_len} / max {max_len} caracteres; se marca solo si la diferencia supera {}% y {OPTION_LENGTH_MIN_ABS_DIFF} caracteres",
                    (OPTION_LENGTH_TOLERANCE * 100.0).round()
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
        StaticVisualSpec::InteractiveModule { spec, challenge, caption } => {
            use crate::domain::notebook::InteractiveModuleSpec as M;
            if let Some(c) = caption {
                scan_text(&format!("{field}.caption"), c, &mut v);
            }
            if let Some(ch) = challenge {
                scan_text(&format!("{field}.challenge.prompt"), &ch.prompt, &mut v);
            }
            match spec {
                M::RelationExplorer { output, variables } => {
                    scan_text(&format!("{field}.output.label"), &output.label, &mut v);
                    variables.iter().for_each(|x| scan_text(&format!("{field}.variables.{}", x.sym), &x.label, &mut v));
                }
                M::FunctionPlot { params, .. } => params.iter().for_each(|x| scan_text(&format!("{field}.params.{}", x.sym), &x.label, &mut v)),
                M::VectorSum { vectors, .. } => vectors.iter().for_each(|x| scan_text(&format!("{field}.vectors"), &x.label, &mut v)),
                M::ParticleGradient { high_label, low_label, .. } => {
                    scan_text(&format!("{field}.highLabel"), high_label, &mut v);
                    scan_text(&format!("{field}.lowLabel"), low_label, &mut v);
                }
                M::Timeline { events } => events.iter().for_each(|e| {
                    scan_text(&format!("{field}.events.label"), &e.label, &mut v);
                    scan_text(&format!("{field}.events.detail"), &e.detail, &mut v);
                }),
            }
        }
        StaticVisualSpec::SequenceStrip { title, rows, caption } => {
            if let Some(t) = title {
                scan_text(&format!("{field}.title"), t, &mut v);
            }
            scan_text(&format!("{field}.caption"), caption, &mut v);
            for (r, row) in rows.iter().enumerate() {
                scan_text(&format!("{field}.rows[{r}].label"), &row.label, &mut v);
                for (c, chunk) in row.chunks.iter().enumerate() {
                    scan_text(&format!("{field}.rows[{r}].chunks[{c}].meaning"), &chunk.meaning, &mut v);
                    if let Some(e) = &chunk.explanation {
                        scan_text(&format!("{field}.rows[{r}].chunks[{c}].explanation"), e, &mut v);
                    }
                }
            }
        }
        StaticVisualSpec::DiagramSpec { caption, title, nodes, .. } => {
            if let Some(c) = caption {
                scan_text(&format!("{field}.caption"), c, &mut v);
            }
            if let Some(t) = title {
                scan_text(&format!("{field}.title"), t, &mut v);
            }
            for (idx, node) in nodes.iter().enumerate() {
                scan_text(&format!("{field}.nodes[{idx}].label"), &node.label, &mut v);
                if let Some(sub) = &node.subtext {
                    scan_text(&format!("{field}.nodes[{idx}].subtext"), sub, &mut v);
                }
                if let Some(note) = &node.explanation {
                    scan_text(&format!("{field}.nodes[{idx}].explanation"), note, &mut v);
                }
            }
        }
        StaticVisualSpec::TopologyFlow { title, nodes, steps, caption, .. } => {
            for text in [caption, title].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
            for (idx, node) in nodes.iter().enumerate() {
                scan_text(&format!("{field}.nodes[{idx}].label"), &node.label, &mut v);
                if let Some(note) = &node.explanation {
                    scan_text(&format!("{field}.nodes[{idx}].explanation"), note, &mut v);
                }
            }
            for (idx, step) in steps.iter().enumerate() {
                scan_text(&format!("{field}.steps[{idx}].title"), &step.title, &mut v);
                scan_text(&format!("{field}.steps[{idx}].detail"), &step.detail, &mut v);
            }
        }
        StaticVisualSpec::ExplorableModel { title, shell, parts, caption } => {
            for text in [caption, title].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
            if let Some(shell) = shell {
                scan_text(&format!("{field}.shell.label"), &shell.label, &mut v);
            }
            for (idx, part) in parts.iter().enumerate() {
                scan_text(&format!("{field}.parts[{idx}].label"), &part.label, &mut v);
                scan_text(&format!("{field}.parts[{idx}].explanation"), &part.explanation, &mut v);
            }
        }
        StaticVisualSpec::Molecule3d { title, steps, caption, .. } => {
            for text in [caption, title].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
            for (idx, s) in steps.iter().enumerate() {
                scan_text(&format!("{field}.steps[{idx}].title"), &s.title, &mut v);
                scan_text(&format!("{field}.steps[{idx}].detail"), &s.detail, &mut v);
            }
        }
        StaticVisualSpec::AlgebraicStructure { title, caption, .. } | StaticVisualSpec::Instrument { title, caption, .. } => {
            for text in [caption, title].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
        }
        StaticVisualSpec::IllustrationKit { title, items, forces, caption, .. } => {
            for text in [caption, title].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
            for (idx, it) in items.iter().enumerate() {
                scan_text(&format!("{field}.items[{idx}].label"), &it.label, &mut v);
                scan_text(&format!("{field}.items[{idx}].explanation"), &it.explanation, &mut v);
            }
            for (idx, fo) in forces.iter().enumerate() {
                scan_text(&format!("{field}.forces[{idx}].label"), &fo.label, &mut v);
            }
        }
        StaticVisualSpec::CodeFigure { title, caption, .. } => {
            scan_text(&format!("{field}.caption"), caption, &mut v);
            if let Some(t) = title {
                scan_text(&format!("{field}.title"), t, &mut v);
            }
        }
        StaticVisualSpec::Surface3d { title, z_label, paths, markers, caption, params, .. } => {
            for text in [caption, title, z_label].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
            for p in params {
                scan_text(&format!("{field}.params.{}", p.sym), &p.label, &mut v);
            }
            for (idx, p) in paths.iter().enumerate() {
                scan_text(&format!("{field}.paths[{idx}].label"), &p.label, &mut v);
                scan_text(&format!("{field}.paths[{idx}].explanation"), &p.explanation, &mut v);
            }
            for (idx, m) in markers.iter().enumerate() {
                scan_text(&format!("{field}.markers[{idx}].label"), &m.label, &mut v);
                scan_text(&format!("{field}.markers[{idx}].explanation"), &m.explanation, &mut v);
            }
        }
        StaticVisualSpec::SolidModel { title, features, caption, .. } => {
            for text in [caption, title].into_iter().flatten() {
                scan_text(&format!("{field}.caption"), text, &mut v);
            }
            for (idx, f) in features.iter().enumerate() {
                scan_text(&format!("{field}.features[{idx}].label"), &f.label, &mut v);
                scan_text(&format!("{field}.features[{idx}].explanation"), &f.explanation, &mut v);
            }
        }
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
    let expected = chart_type.headers();
    if !expected.contains(&first_token) {
        v.push(format!(
            "bloque: {field} mermaid no abre con el tipo declarado ({} debería empezar con \"{}\") — empieza con \"{first_token}\"",
            chart_type.as_str(),
            expected.join("\" | \"")
        ));
    }
    v.extend(mermaid_shape_violations(chart_type, trimmed, field));
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

/// The one thing each newer chart type cannot render without — checked here
/// so the model fixes it in the same turn instead of the student seeing a
/// Mermaid parse error. Deliberately minimal: the header, the bracket and
/// quote balance above, plus each type's skeleton.
fn mermaid_shape_violations(chart_type: MermaidChartType, code: &str, field: &str) -> Vec<String> {
    let body: Vec<&str> = code.lines().skip(1).map(str::trim_end).filter(|l| !l.trim().is_empty() && !l.trim().starts_with("%%")).collect();
    let lower = code.to_lowercase();
    let mut missing: Vec<&str> = Vec::new();
    match chart_type {
        MermaidChartType::Gantt => {
            if !lower.contains("dateformat") {
                missing.push("`dateFormat YYYY-MM-DD`");
            }
            if !body.iter().any(|l| l.contains(':') && !l.trim_start().to_lowercase().starts_with("title")) {
                missing.push("al menos una tarea `Nombre :id, 2025-01-06, 5d`");
            }
        }
        MermaidChartType::Pie => {
            let data: Vec<&&str> = body.iter().filter(|l| !l.trim_start().to_lowercase().starts_with("title")).collect();
            let well_formed = |l: &str| {
                let l = l.trim();
                l.starts_with('"') && l.rsplit_once(':').is_some_and(|(label, n)| label.trim_end().ends_with('"') && n.trim().parse::<f64>().is_ok_and(|x| x >= 0.0))
            };
            if data.len() < 2 || !data.iter().all(|l| well_formed(l)) {
                missing.push("2+ líneas `\"Etiqueta\" : número`");
            }
        }
        MermaidChartType::Mindmap => {
            let indent = |l: &&str| l.len() - l.trim_start().len();
            let min = body.iter().map(indent).min().unwrap_or(0);
            if body.is_empty() || body.iter().filter(|l| indent(l) == min).count() != 1 {
                missing.push("UNA sola raíz (`root((Idea))`) con las ramas más indentadas debajo");
            }
        }
        MermaidChartType::Timeline => {
            if body.iter().filter(|l| l.contains(':') && !l.trim_start().to_lowercase().starts_with("title")).count() < 2 {
                missing.push("2+ eventos `periodo : evento`");
            }
        }
        MermaidChartType::QuadrantChart => {
            for axis in ["x-axis", "y-axis"] {
                if !lower.contains(axis) {
                    missing.push(if axis == "x-axis" { "`x-axis Bajo --> Alto`" } else { "`y-axis Bajo --> Alto`" });
                }
            }
            if !body.iter().any(|l| l.contains(": [")) {
                missing.push("puntos `Nombre: [0.3, 0.6]`");
            }
        }
        MermaidChartType::XyChart => {
            if !lower.contains("x-axis") {
                missing.push("`x-axis [ene, feb, mar]`");
            }
            if !body.iter().any(|l| {
                let t = l.trim_start().to_lowercase();
                (t.starts_with("bar") || t.starts_with("line")) && t.contains('[')
            }) {
                missing.push("una serie `bar [1, 2, 3]` o `line [1, 2, 3]`");
            }
        }
        MermaidChartType::Sankey => {
            let rows = body.iter().filter(|l| l.split(',').count() == 3 && l.rsplit(',').next().is_some_and(|n| n.trim().parse::<f64>().is_ok())).count();
            if rows == 0 {
                missing.push("filas CSV `origen,destino,valor`");
            }
        }
        MermaidChartType::Journey => {
            if !body.iter().any(|l| l.trim_start().to_lowercase().starts_with("section")) {
                missing.push("`section Etapa` con tareas `Tarea: 3: Actor`");
            }
        }
        MermaidChartType::GitGraph => {
            if !lower.contains("commit") {
                missing.push("`commit` / `branch` / `checkout` / `merge`");
            }
        }
        _ => {}
    }
    if missing.is_empty() {
        return Vec::new();
    }
    vec![format!("bloque: {field} mermaid {} no se puede dibujar: le falta {}", chart_type.as_str(), missing.join(" y "))]
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
            interaction_mode: None,
            shuffled_steps: None,
            correct_order: None,
            boundary_cases: None,
            visual_aid: None,
        }
    }

    fn valid_theory() -> GeneratedSectionBlock {
        GeneratedSectionBlock::AnchoredMicroTheory {
            title: "La gran imagen".to_string(),
            intuitive_hook: "Piensa en una fábrica circular".to_string(),
            analogy_boundary: Some("La fábrica no explica qué pasa cuando el buffer se llena".to_string()),
            system_rule: "Explicación breve del tema.".to_string(),
            frequent_error: "Confundir la entrada con la salida".to_string(),
            visual_aid: None,
            visual_intent: None,
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
    fn a_20_character_gap_between_short_options_is_not_length_bias() {
        // 36 vs 56 chars: a 36% ratio, but only 20 chars apart.
        let mut block = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { options, .. } = &mut block {
            options[0] = "a".repeat(36);
            options[1] = "b".repeat(56);
        }
        let v = block_guardrail_violations(&block);
        assert!(!v.iter().any(|s| s.contains("sesgo de longitud")), "{v:?}");
    }

    #[test]
    fn three_options_of_36_54_and_56_characters_are_not_length_bias() {
        // Spread 20 chars (36%): over the ratio but under the 25-char gap.
        let mut block = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { options, .. } = &mut block {
            *options = vec!["a".repeat(36), "b".repeat(54), "c".repeat(56)];
        }
        let v = block_guardrail_violations(&block);
        assert!(!v.iter().any(|s| s.contains("sesgo de longitud")), "{v:?}");
    }

    #[test]
    fn a_clearly_biased_pair_is_still_flagged_with_thresholds_from_constants() {
        let mut block = valid_gate();
        if let GeneratedSectionBlock::InteractivePredictionGate { options, .. } = &mut block {
            options[0] = "a".repeat(20);
            options[1] = "b".repeat(100);
        }
        let v = block_guardrail_violations(&block);
        let msg = v.iter().find(|s| s.contains("sesgo de longitud")).expect("flagged");
        assert!(msg.contains("25%") && msg.contains("25 caracteres"), "{msg}");
    }

    #[test]
    fn length_bias_needs_both_the_ratio_and_the_absolute_gap() {
        // Ratio fine (1000 vs 900 = 10%) but 100 chars apart: not biased.
        assert!(!options_are_length_biased(900, 1000));
        // Absolute gap over 25 but ratio exactly at the limit (75 vs 100 = 25%): not biased (strict >).
        assert!(!options_are_length_biased(75, 100));
        // Ratio over 25% but gap exactly 25 chars (45 vs 70 = 35.7%): not biased (strict >).
        assert!(!options_are_length_biased(45, 70));
        // Both just over: 26 chars, 26/100 = 26%.
        assert!(options_are_length_biased(74, 100));
        assert!(!options_are_length_biased(0, 0));
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
            analogy_boundary: None,
            system_rule: "R".to_string(),
            frequent_error: "E".to_string(),
            visual_aid: None,
            visual_intent: None,
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
        assert!(needs_llm_critic(DynamicBlockType::DeclarativeVisualDiagram), "the figure must agree with the class");
        assert!(needs_llm_critic(DynamicBlockType::HeuristicErrorAudit));
        assert!(needs_llm_critic(DynamicBlockType::HandsOnMission));
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
            visual_intent: None,
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

    #[test]
    fn a_transfer_mission_must_have_no_hints_but_an_ordinary_one_is_unaffected() {
        let mission = |transfer: bool, hints: Vec<String>| GeneratedSectionBlock::HandsOnMission {
            challenge_statement: "Aplica la idea en otro dominio".to_string(),
            expected_milestone_artifact: "Una solución".to_string(),
            constraints: vec!["Datos incompletos".to_string()],
            scaffolding_hints: hints,
            evaluation_rubric_summary: vec!["Adapta la regla".to_string()],
            is_transfer: transfer,
            support_level: None,
            worked_template: None,
            interactive_steps: None,
            visual_aid: None,
        };
        let hint = vec!["Empieza por el caso base".to_string()];
        assert!(block_guardrail_violations(&mission(true, hint.clone())).iter().any(|m| m.contains("transferencia")));
        assert!(block_guardrail_violations(&mission(true, vec![])).iter().all(|m| !m.contains("transferencia")));
        assert!(block_guardrail_violations(&mission(false, hint)).iter().all(|m| !m.contains("transferencia")));
    }

    #[test]
    fn test_evaluate_options_length_slack_tiers() {
        // Tier 1: Optimal (exact or <= 15%)
        let opt1 = vec!["Diez palabras para la primera opción de respuesta corta".to_string(), "Diez palabras para la segunda opción de respuesta corta".to_string()];
        assert_eq!(evaluate_options_length_slack(&opt1), ValidationSeverity::Optimal);

        // Tier 2: Warning with Slack (51 vs 61 chars, 16.4% relative diff > 15%, but diff 10 <= 25 chars)
        let opt2 = vec![
            "Esta opción cuenta con exactamente cincuenta y un c".to_string(),
            "Esta otra alternativa posee sesenta y un caracteres en total.".to_string(),
        ];
        assert_eq!(evaluate_options_length_slack(&opt2), ValidationSeverity::Warning);

        // Tier 3: Critical (20 chars vs 100 chars, severe bias)
        let opt3 = vec!["a".repeat(20), "b".repeat(100)];
        assert_eq!(evaluate_options_length_slack(&opt3), ValidationSeverity::Critical);
    }
}

#[cfg(test)]
mod mermaid_shape_tests {
    use super::*;

    fn issues(t: MermaidChartType, code: &str) -> Vec<String> {
        mermaid_violations(t, code, "visualAid")
    }

    #[test]
    fn each_newer_chart_type_needs_its_skeleton() {
        let good = [
            (MermaidChartType::Mindmap, "mindmap\n  root((CAD))\n    Croquis\n      Restricciones\n    Operaciones"),
            (MermaidChartType::Timeline, "timeline\n  title Historia del CAD\n  1963 : Sketchpad\n  1982 : AutoCAD"),
            (MermaidChartType::Gantt, "gantt\n  dateFormat YYYY-MM-DD\n  section Diseño\n  Bocetos :a1, 2025-01-06, 5d"),
            (MermaidChartType::Pie, "pie title Material\n  \"PLA\" : 60\n  \"PETG\" : 40"),
            (MermaidChartType::QuadrantChart, "quadrantChart\n  x-axis Bajo --> Alto\n  y-axis Bajo --> Alto\n  Tarea A: [0.3, 0.8]"),
            (MermaidChartType::XyChart, "xychart-beta\n  x-axis [ene, feb]\n  bar [20, 45]"),
            (MermaidChartType::Sankey, "sankey-beta\n  Solar,Red,40\n  Red,Hogares,30"),
            (MermaidChartType::Journey, "journey\n  section Comprar\n    Buscar: 3: Cliente"),
            (MermaidChartType::GitGraph, "gitGraph\n  commit\n  branch feature\n  commit"),
        ];
        for (t, code) in good {
            assert!(issues(t, code).is_empty(), "{} -> {:?}", t.as_str(), issues(t, code));
        }
        assert!(issues(MermaidChartType::Mindmap, "mindmap\n  CAD\n  Croquis").iter().any(|s| s.contains("UNA sola raíz")));
        assert!(issues(MermaidChartType::Gantt, "gantt\n  section Diseño\n  Bocetos :a1, 2025-01-06, 5d").iter().any(|s| s.contains("dateFormat")));
        assert!(issues(MermaidChartType::Pie, "pie\n  PLA 60\n  PETG 40").iter().any(|s| s.contains("Etiqueta")));
        assert!(issues(MermaidChartType::Timeline, "mindmap\n  1963 : Sketchpad\n  1982 : AutoCAD").iter().any(|s| s.contains("no abre con el tipo")));
    }
}
