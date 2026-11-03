//! Deterministic repair of a generated visual BEFORE the review runs.
//!
//! Most of what the guardrails reject in a visual is mechanical: a prop in
//! kebab-case, a `text` with no background pill, a diagram node id with a
//! space, an edge pointing at a label instead of an id, a slider whose initial
//! value sits outside its range. Each of those used to cost a whole extra
//! generation (seconds of the student's time and thousands of output tokens)
//! to be fixed by the model; here they are fixed in microseconds. Only things
//! whose intent is unambiguous are touched — anything else is left for the
//! guardrails to report, so repair never hides a real defect.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::domain::notebook::{GeneratedSectionBlock, InteractiveModuleSpec, ModuleVariable, StaticVisualSpec, SvgElement, SvgTag};

/// Fixes applied to the block's visual (0 = untouched).
pub fn repair_block(block: &mut GeneratedSectionBlock) -> usize {
    match visual_mut(block) {
        Some(visual) => repair_visual(visual),
        None => 0,
    }
}

fn visual_mut(block: &mut GeneratedSectionBlock) -> Option<&mut StaticVisualSpec> {
    match block {
        GeneratedSectionBlock::DeclarativeVisualDiagram { visual_aid, .. } => Some(visual_aid),
        GeneratedSectionBlock::BranchingScenarioChallenge { visual_aid, .. }
        | GeneratedSectionBlock::HeuristicErrorAudit { visual_aid, .. }
        | GeneratedSectionBlock::InteractivePredictionGate { visual_aid, .. }
        | GeneratedSectionBlock::HandsOnMission { visual_aid, .. } => visual_aid.as_mut(),
        _ => None,
    }
}

pub fn repair_visual(visual: &mut StaticVisualSpec) -> usize {
    match visual {
        StaticVisualSpec::DeclarativeSvg { view_box, elements, groups, .. } => {
            let mut fixes = 0;
            let valid = {
                let t: Vec<&str> = view_box.split_whitespace().collect();
                t.len() == 4 && t.iter().all(|x| x.parse::<f64>().is_ok())
            };
            if !valid {
                *view_box = "0 0 800 450".to_string();
                fixes += 1;
            }
            fixes += repair_svg_elements(elements);
            for g in groups.iter_mut() {
                fixes += repair_svg_elements(&mut g.elements);
            }
            fixes
        }
        StaticVisualSpec::DiagramSpec { nodes, edges, .. } => repair_diagram(nodes, edges),
        StaticVisualSpec::InteractiveModule { spec, .. } => repair_module(spec),
        _ => 0,
    }
}

// --- declarative_svg ---------------------------------------------------------

/// `fill-opacity` -> `fillOpacity` (React props are camelCase). `data-*` and
/// `aria-*` keep their dashes.
fn camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for c in key.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn repair_svg_elements(elements: &mut Vec<SvgElement>) -> usize {
    let mut fixes = 0;
    for el in elements.iter_mut() {
        if let Some(props) = el.props.as_object_mut() {
            let dashed: Vec<String> = props.keys().filter(|k| k.contains('-') && !k.starts_with("data-") && !k.starts_with("aria-")).cloned().collect();
            for key in dashed {
                if let Some(v) = props.remove(&key) {
                    props.entry(camel(&key)).or_insert(v);
                    fixes += 1;
                }
            }
        }
    }
    // A text that is not right after a rect gets its background pill.
    let mut i = 0;
    while i < elements.len() {
        let needs_pill = elements[i].tag == SvgTag::Text && !(i > 0 && elements[i - 1].tag == SvgTag::Rect);
        if needs_pill {
            let pill = pill_for(&elements[i]);
            elements.insert(i, pill);
            fixes += 1;
            i += 1; // skip over the text we just padded
        }
        i += 1;
    }
    fixes
}

fn num(v: Option<&Value>, default: f64) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
        _ => default,
    }
}

/// The dark, slightly transparent pill the prompt asks for, sized from the text.
fn pill_for(text: &SvgElement) -> SvgElement {
    let props = text.props.as_object();
    let get = |k: &str| props.and_then(|p| p.get(k));
    let font = num(get("fontSize"), 14.0).clamp(6.0, 64.0);
    let (x, y) = (num(get("x"), 0.0), num(get("y"), 0.0));
    let width = text.label.as_deref().unwrap_or("").chars().count() as f64 * font * 0.56 + 8.0;
    let x0 = match get("textAnchor").and_then(Value::as_str) {
        Some("middle") => x - width / 2.0,
        Some("end") => x - width,
        _ => x - 4.0,
    };
    SvgElement {
        tag: SvgTag::Rect,
        props: json!({ "x": round1(x0), "y": round1(y - font * 0.8 - 3.0), "width": round1(width), "height": round1(font * 1.2 + 6.0), "rx": 4, "fill": "#1e293b", "fillOpacity": 0.85 }),
        label: None,
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

// --- diagram_spec --------------------------------------------------------------

fn sanitize_id(raw: &str) -> String {
    let mut id: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    id = id.trim_matches('_').to_string();
    if id.is_empty() {
        id = "n".to_string();
    }
    if id.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        id.insert(0, 'n');
    }
    id
}

fn repair_diagram(nodes: &mut Vec<crate::domain::notebook::DiagramNodeSpec>, edges: &mut Vec<crate::domain::notebook::DiagramEdgeSpec>) -> usize {
    let mut fixes = 0;
    // Valid, unique ids; remember how each original id was renamed.
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut used: HashSet<String> = HashSet::new();
    for node in nodes.iter_mut() {
        let original = node.id.clone();
        let mut id = sanitize_id(&original);
        let base = id.clone();
        let mut n = 2;
        while used.contains(&id) {
            id = format!("{base}{n}");
            n += 1;
        }
        if id != original {
            fixes += 1;
        }
        used.insert(id.clone());
        renamed.entry(original).or_insert_with(|| id.clone());
        node.id = id;
    }
    let by_label: HashMap<String, String> = nodes.iter().map(|n| (n.label.trim().to_lowercase(), n.id.clone())).collect();
    let resolve = |end: &str| -> Option<String> {
        if used.contains(end) {
            return Some(end.to_string());
        }
        renamed.get(end).cloned().or_else(|| by_label.get(&end.trim().to_lowercase()).cloned())
    };
    let before = edges.len();
    for edge in edges.iter_mut() {
        if let (Some(from), Some(to)) = (resolve(&edge.from), resolve(&edge.to)) {
            if from != edge.from || to != edge.to {
                fixes += 1;
            }
            edge.from = from;
            edge.to = to;
        }
    }
    // An edge that still points nowhere cannot be drawn: drop it.
    edges.retain(|e| used.contains(&e.from) && used.contains(&e.to));
    fixes + (before - edges.len())
}

// --- interactive_module ----------------------------------------------------------

fn repair_var(v: &mut ModuleVariable) -> usize {
    let mut fixes = 0;
    if v.min > v.max {
        std::mem::swap(&mut v.min, &mut v.max);
        fixes += 1;
    }
    if v.min.is_finite() && v.max.is_finite() && v.min < v.max {
        let clamped = v.init.clamp(v.min, v.max);
        if clamped != v.init {
            v.init = clamped;
            fixes += 1;
        }
        if v.step.is_some_and(|s| !(s > 0.0) || s > v.max - v.min) {
            v.step = None;
            fixes += 1;
        }
    }
    fixes
}

fn repair_module(spec: &mut InteractiveModuleSpec) -> usize {
    match spec {
        InteractiveModuleSpec::RelationExplorer { variables, .. } => variables.iter_mut().map(repair_var).sum(),
        InteractiveModuleSpec::FunctionPlot { params, domain, .. } => {
            let mut fixes: usize = params.iter_mut().map(repair_var).sum();
            if domain[0] > domain[1] {
                domain.swap(0, 1);
                fixes += 1;
            }
            fixes
        }
        InteractiveModuleSpec::ParticleGradient { high_count, low_count, has_barrier, channels, .. } => {
            let mut fixes = 0;
            if *high_count < *low_count {
                std::mem::swap(high_count, low_count);
                fixes += 1;
            }
            if *has_barrier && *channels == 0 {
                *channels = 1;
                fixes += 1;
            }
            fixes
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::{DiagramEdgeSpec, DiagramNodeSpec};
    use crate::domain::pedagogy_guardrails::block_guardrail_violations;

    fn el(tag: SvgTag, props: Value, label: Option<&str>) -> SvgElement {
        SvgElement { tag, props, label: label.map(String::from) }
    }

    fn svg_block(elements: Vec<SvgElement>) -> GeneratedSectionBlock {
        GeneratedSectionBlock::DeclarativeVisualDiagram {
            title: "t".into(),
            visual_aid: StaticVisualSpec::DeclarativeSvg { view_box: "0 0 800 450".into(), elements, groups: vec![], pedagogical_focus: None, caption: "c".into() },
            guided_walkthrough: vec![crate::domain::notebook::WalkthroughStep { step_number: 1, target_visual_element: "x".into(), pedagogical_insight: "y".into() }],
        }
    }

    #[test]
    fn kebab_props_and_missing_pills_are_fixed_and_the_guardrails_then_pass() {
        let mut block = svg_block(vec![
            el(SvgTag::Line, json!({"x1": 0, "x2": 9, "stroke-dasharray": "3 3", "data-keep": 1}), None),
            el(SvgTag::Text, json!({"x": 100, "y": 50, "fontSize": 14, "textAnchor": "middle", "fill-opacity": 1}), Some("Membrana")),
        ]);
        assert!(!block_guardrail_violations(&block).is_empty(), "the raw block breaks the rules");
        let fixes = repair_block(&mut block);
        assert_eq!(fixes, 3, "two kebab props + one pill");
        assert_eq!(block_guardrail_violations(&block), Vec::<String>::new());
        let GeneratedSectionBlock::DeclarativeVisualDiagram { visual_aid: StaticVisualSpec::DeclarativeSvg { elements, .. }, .. } = &block else { panic!() };
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].props["strokeDasharray"], "3 3");
        assert_eq!(elements[0].props["data-keep"], 1, "data-* keeps its dash");
        assert_eq!(elements[1].tag, SvgTag::Rect);
        assert_eq!(elements[2].tag, SvgTag::Text);
        assert!(elements[1].props["width"].as_f64().unwrap() > 40.0, "pill is sized from the text");
    }

    #[test]
    fn already_clean_visuals_are_left_alone() {
        let mut block = svg_block(vec![el(SvgTag::Rect, json!({"x": 0, "y": 0, "width": 10, "height": 10}), None), el(SvgTag::Text, json!({"x": 1, "y": 1}), Some("a"))]);
        assert_eq!(repair_block(&mut block), 0);
    }

    #[test]
    fn an_invalid_view_box_is_reset() {
        let mut v = StaticVisualSpec::DeclarativeSvg { view_box: "banana".into(), elements: vec![], groups: vec![], pedagogical_focus: None, caption: "c".into() };
        assert_eq!(repair_visual(&mut v), 1);
        let StaticVisualSpec::DeclarativeSvg { view_box, .. } = v else { panic!() };
        assert_eq!(view_box, "0 0 800 450");
    }

    #[test]
    fn diagram_ids_are_made_valid_and_edges_follow_them() {
        let node = |id: &str, label: &str| DiagramNodeSpec { id: id.into(), label: label.into(), shape: None, subtext: None, explanation: None };
        let edge = |from: &str, to: &str| DiagramEdgeSpec { from: from.into(), to: to.into(), label: None, style: None };
        let mut v = StaticVisualSpec::DiagramSpec {
            kind: "flowchart".into(),
            direction: None,
            title: None,
            nodes: vec![node("Cache Redis", "Caché"), node("db", "Base de datos"), node("db", "Réplica"), node("1st", "Primero")],
            // by renamed original id, by label, to a duplicate, and to nothing at all
            edges: vec![edge("Cache Redis", "db"), edge("Base de datos", "Caché"), edge("1st", "db"), edge("db", "ghost")],
            caption: None,
        };
        let fixes = repair_visual(&mut v);
        assert!(fixes >= 4, "{fixes}");
        let StaticVisualSpec::DiagramSpec { nodes, edges, .. } = &v else { panic!() };
        assert_eq!(nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), vec!["Cache_Redis", "db", "db2", "n1st"]);
        assert_eq!(edges.len(), 3, "the edge to a node that does not exist is dropped");
        assert_eq!((edges[0].from.as_str(), edges[0].to.as_str()), ("Cache_Redis", "db"));
        assert_eq!((edges[1].from.as_str(), edges[1].to.as_str()), ("db", "Cache_Redis"), "labels resolve to ids");
    }

    #[test]
    fn module_sliders_are_clamped_and_swapped_only_when_unambiguous() {
        let var = |min: f64, max: f64, init: f64, step: Option<f64>| ModuleVariable { sym: "k".into(), label: "K".into(), unit: None, min, max, step, init };
        let mut v = var(10.0, 0.0, 99.0, Some(500.0));
        assert_eq!(repair_var(&mut v), 3);
        assert_eq!((v.min, v.max, v.init, v.step), (0.0, 10.0, 10.0, None));
        let mut ok = var(0.0, 1.0, 0.5, Some(0.1));
        assert_eq!(repair_var(&mut ok), 0);

        let mut pg = InteractiveModuleSpec::ParticleGradient { high_label: "a".into(), low_label: "b".into(), high_count: 3, low_count: 9, has_barrier: true, channels: 0 };
        assert_eq!(repair_module(&mut pg), 2);
        let InteractiveModuleSpec::ParticleGradient { high_count, low_count, channels, .. } = pg else { panic!() };
        assert_eq!((high_count, low_count, channels), (9, 3, 1));
    }

    #[test]
    fn blocks_without_a_visual_cost_nothing() {
        let mut b = GeneratedSectionBlock::MetacognitiveClosure {
            synthesis_task: "s".into(),
            prediction_comparison: crate::domain::notebook::PredictionComparison { initial_prediction: "a".into(), final_result: "b".into(), contrast_narrative: "c".into() },
            self_evaluation_checklist: vec![],
        };
        assert_eq!(repair_block(&mut b), 0);
    }
}
