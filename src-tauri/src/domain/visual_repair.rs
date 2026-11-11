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
    let mut fixes = match visual_mut(block) {
        Some(visual) => repair_visual(visual),
        None => 0,
    };
    if let GeneratedSectionBlock::DeclarativeVisualDiagram { visual_aid, guided_walkthrough, .. } = block {
        fixes += repair_walkthrough(visual_aid, guided_walkthrough);
    }
    fixes += repair_intent(block);
    fixes
}

/// "Spatial Shape", "3d", "proceso" -> the intent key; a word that maps to no
/// intent is dropped (the review then judges the figure on its own).
fn repair_intent(block: &mut GeneratedSectionBlock) -> usize {
    let slot = match block {
        GeneratedSectionBlock::AnchoredMicroTheory { visual_intent, .. } | GeneratedSectionBlock::DeclarativeVisualDiagram { visual_intent, .. } => {
            visual_intent
        }
        _ => return 0,
    };
    let Some(raw) = slot.as_deref() else { return 0 };
    let fixed = crate::domain::visual_routing::normalize_intent(raw).map(str::to_string);
    if fixed.as_deref() == Some(raw) {
        return 0;
    }
    *slot = fixed;
    1
}

// --- guided walkthrough --------------------------------------------------------

/// Lower-case, accents folded, only letters and digits: "Núcleo externo" and
/// "nucleo_externo" compare equal.
pub(crate) fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Points every walkthrough step at something the scene can actually
/// highlight. The model often names a part by its label ("Núcleo externo"),
/// the envelope ("shell", "Membrana"), or, in a topology, a message ("SYN
/// (seq=100)") or a step title instead of a node id; the frontend then has
/// nothing to light up and the step reads as decoration. Only unambiguous
/// matches are rewritten; anything else is left as written.
fn repair_walkthrough(visual: &StaticVisualSpec, steps: &mut [crate::domain::notebook::WalkthroughStep]) -> usize {
    use crate::domain::notebook::SHELL_TARGET;
    // (id, label) of every highlightable element, plus (folded alias, id).
    let mut targets: Vec<(String, String)> = Vec::new();
    let mut aliases: Vec<(String, String)> = Vec::new();
    match visual {
        StaticVisualSpec::TopologyFlow { nodes, steps: flow, .. } => {
            targets.extend(nodes.iter().map(|n| (n.id.clone(), n.label.clone())));
            // A message lights up where it lands; a step, where its first message lands.
            for s in flow {
                if let Some(first) = s.messages.first() {
                    aliases.push((fold(&s.title), first.to.clone()));
                }
                for m in &s.messages {
                    aliases.push((fold(&m.label), m.to.clone()));
                }
            }
        }
        StaticVisualSpec::ExplorableModel { shell, parts, .. } => {
            targets.extend(parts.iter().map(|p| (p.id.clone(), p.label.clone())));
            if let Some(shell) = shell {
                for alias in [SHELL_TARGET, "envoltura", "envelope", shell.label.as_str()] {
                    aliases.push((fold(alias), SHELL_TARGET.to_string()));
                }
            }
        }
        StaticVisualSpec::DiagramSpec { nodes, .. } => targets.extend(nodes.iter().map(|n| (n.id.clone(), n.label.clone()))),
        StaticVisualSpec::IllustrationKit { items, .. } => targets.extend(items.iter().map(|i| (i.id.clone(), i.label.clone()))),
        StaticVisualSpec::Molecule3d { atoms, .. } => targets.extend(atoms.iter().map(|a| (a.id.clone(), a.label.clone().unwrap_or_else(|| a.element.clone())))),
        StaticVisualSpec::Surface3d { paths, markers, .. } => {
            targets.extend(paths.iter().map(|p| (p.id.clone(), p.label.clone())));
            targets.extend(markers.iter().map(|m| (m.id.clone(), m.label.clone())));
        }
        StaticVisualSpec::SolidModel { features, .. } => {
            use crate::domain::notebook::{CAMERA_TARGET, SOLID_VIEWS};
            targets.extend(features.iter().map(|f| (f.id.clone(), f.label.clone())));
            targets.extend(SOLID_VIEWS.iter().chain([&CAMERA_TARGET]).map(|v| (v.to_string(), v.to_string())));
            for (alias, view) in [
                ("isometrica", "iso"),
                ("vista isometrica", "iso"),
                ("frontal", "front"),
                ("vista frontal", "front"),
                ("alzado", "front"),
                ("superior", "top"),
                ("vista superior", "top"),
                ("planta", "top"),
                ("lateral", "right"),
                ("vista lateral", "right"),
                ("lateral derecha", "right"),
                ("perfil", "right"),
                ("camara", CAMERA_TARGET),
                ("tu camara", CAMERA_TARGET),
            ] {
                aliases.push((fold(alias), view.to_string()));
            }
        }
        _ => return 0,
    }
    let has_shell = aliases.iter().any(|(_, t)| t == SHELL_TARGET);
    let resolve = |raw: &str| -> Option<String> {
        if targets.iter().any(|(id, _)| id == raw) || (raw == SHELL_TARGET && has_shell) {
            return None; // already exact
        }
        let wanted = fold(raw);
        if wanted.is_empty() {
            return None;
        }
        if let Some((id, _)) = targets.iter().find(|(id, label)| fold(id) == wanted || fold(label) == wanted) {
            return Some(id.clone());
        }
        if let Some((_, id)) = aliases.iter().find(|(alias, _)| *alias == wanted) {
            return Some(id.clone());
        }
        // Last resort: exactly ONE element whose id/label contains the target
        // or is contained by it ("el manto terrestre" -> manto).
        let hits: Vec<&String> = targets
            .iter()
            .filter(|(id, label)| {
                [fold(id), fold(label)]
                    .iter()
                    .any(|t| t.len() >= 3 && (wanted.contains(t.as_str()) || (wanted.len() >= 3 && t.contains(wanted.as_str()))))
            })
            .map(|(id, _)| id)
            .collect();
        match hits.as_slice() {
            [only] => Some((*only).clone()),
            _ => None,
        }
    };
    let mut fixes = 0;
    for step in steps.iter_mut() {
        if let Some(id) = resolve(&step.target_visual_element) {
            step.target_visual_element = id;
            fixes += 1;
        }
    }
    fixes
}

fn visual_mut(block: &mut GeneratedSectionBlock) -> Option<&mut StaticVisualSpec> {
    match block {
        GeneratedSectionBlock::DeclarativeVisualDiagram { visual_aid, .. } => Some(visual_aid),
        GeneratedSectionBlock::AnchoredMicroTheory { visual_aid, .. }
        | GeneratedSectionBlock::BranchingScenarioChallenge { visual_aid, .. }
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
        StaticVisualSpec::TopologyFlow { nodes, links, steps, .. } => repair_topology(nodes, links, steps),
        StaticVisualSpec::ExplorableModel { shell, parts, .. } => repair_model(shell.as_mut(), parts),
        StaticVisualSpec::SolidModel { features, start_view, units, .. } => repair_solid(features, start_view, units),
        StaticVisualSpec::Molecule3d { atoms, .. } => {
            let mut fixes = 0;
            for a in atoms.iter_mut() {
                // "cl" / "CL" -> "Cl": element symbols are capitalised.
                let t = a.element.trim();
                let fixed: String = t.chars().take(1).flat_map(char::to_uppercase).chain(t.chars().skip(1).flat_map(char::to_lowercase)).collect();
                if fixed != a.element {
                    a.element = fixed;
                    fixes += 1;
                }
            }
            fixes
        }
        StaticVisualSpec::AlgebraicStructure { group, view, .. } => {
            use crate::domain::notebook::GROUP_VIEWS;
            let mut fixes = 0;
            let kind = match group.kind.trim().to_lowercase().as_str() {
                "z" | "zn" | "z_n" | "ciclico" | "cíclico" | "cyclic_group" => "cyclic".to_string(),
                "d" | "dn" | "d_n" | "diedral" | "diédrico" | "diedrico" | "dihedral_group" => "dihedral".to_string(),
                "s" | "sn" | "s_n" | "simetrico" | "simétrico" | "symmetric_group" | "permutations" => "symmetric".to_string(),
                other => other.to_string(),
            };
            if kind != group.kind {
                group.kind = kind;
                fixes += 1;
            }
            fixes += snap(view, &GROUP_VIEWS, "both");
            fixes
        }
        StaticVisualSpec::Instrument { instrument, chords, progression, strum, .. } => {
            let mut fixes = snap(instrument, &crate::domain::instrument::INSTRUMENTS, "guitar");
            for c in chords.iter_mut().chain(progression.iter_mut().map(|b| &mut b.chord)) {
                let fixed = english_chord(c);
                if fixed != *c {
                    *c = fixed;
                    fixes += 1;
                }
            }
            if let Some(s) = strum {
                let fixed: String = s
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .map(|c| match c {
                        'd' | '↓' | 'B' | 'b' => 'D',
                        'u' | '↑' | 'A' | 'a' => 'U',
                        'x' | 'X' | '_' | '.' | '·' => '-',
                        c => c,
                    })
                    .collect();
                if fixed != *s {
                    *s = fixed;
                    fixes += 1;
                }
            }
            fixes
        }
        StaticVisualSpec::IllustrationKit { items, links, forces, .. } => {
            use crate::domain::illustration::{ILLUSTRATION_LINKS, ILLUSTRATION_PIECES};
            let mut fixes = 0;
            for it in items.iter_mut() {
                let k = match it.kind.trim().to_lowercase().as_str() {
                    "bloque" | "caja" | "box" | "mass" | "masa" => "block",
                    "bola" | "pelota" | "esfera" | "sphere" => "ball",
                    "polea" => "pulley",
                    "plano inclinado" | "plano_inclinado" | "rampa" | "ramp" | "slope" => "incline",
                    "pared" | "techo" | "ceiling" | "support" | "soporte" => "wall",
                    "suelo" | "piso" | "floor" => "ground",
                    "bateria" | "batería" | "pila" | "fuente" => "battery",
                    "resistencia" | "resistor" => "resistor",
                    "bombilla" | "foco" | "lampara" | "lámpara" | "lamp" => "bulb",
                    "interruptor" => "switch",
                    "condensador" | "capacitor" => "capacitor",
                    "iman" | "imán" => "magnet",
                    "nodo" | "punto" | "point" => "node",
                    "etiqueta" | "texto" | "text" => "label",
                    other => other,
                }
                .to_string();
                if k != it.kind {
                    it.kind = k;
                    fixes += 1;
                }
                fixes += snap(&mut it.kind, &ILLUSTRATION_PIECES, "block");
            }
            for l in links.iter_mut() {
                let k = match l.kind.trim().to_lowercase().as_str() {
                    "cuerda" | "cable_tension" | "string" | "cord" => "rope",
                    "resorte" | "muelle" => "spring",
                    "cable" | "conductor" => "wire",
                    "barra" | "varilla" => "rod",
                    "flecha" => "arrow",
                    other => other,
                }
                .to_string();
                if k != l.kind {
                    l.kind = k;
                    fixes += 1;
                }
                fixes += snap(&mut l.kind, &ILLUSTRATION_LINKS, "rope");
            }
            for f in forces.iter_mut() {
                let d = match f.direction.trim().to_lowercase().as_str() {
                    "arriba" => "up",
                    "abajo" => "down",
                    "izquierda" => "left",
                    "derecha" => "right",
                    other => other,
                }
                .to_string();
                if d != f.direction {
                    f.direction = d;
                    fixes += 1;
                }
                let m = f.magnitude.clamp(1, 3);
                if m != f.magnitude {
                    f.magnitude = m;
                    fixes += 1;
                }
            }
            fixes
        }
        StaticVisualSpec::Surface3d { paths, markers, .. } => {
            use crate::domain::notebook::SURFACE_PATH_KINDS;
            let mut fixes = 0;
            repair_ids(paths.iter_mut().map(|p| &mut p.id).chain(markers.iter_mut().map(|m| &mut m.id)), &mut fixes);
            for p in paths.iter_mut() {
                let kind = match p.kind.trim().to_lowercase().as_str() {
                    "geodesica" | "geodésica" | "straight" => "geodesic".to_string(),
                    "gradient" | "descenso" | "rolling" | "roll" => "descent".to_string(),
                    "contour" | "contorno" | "nivel" | "level_curve" => "level".to_string(),
                    other => other.to_string(),
                };
                if kind != p.kind {
                    p.kind = kind;
                    fixes += 1;
                }
                fixes += snap(&mut p.kind, &SURFACE_PATH_KINDS, "geodesic");
            }
            fixes
        }
        StaticVisualSpec::Mermaid { chart_type, code, .. } => {
            // The header is the truth Mermaid renders by: a `chartType` that
            // disagrees with it is a typo, not a different diagram.
            match crate::domain::notebook::MermaidChartType::from_code(code) {
                Some(declared) if declared != *chart_type => {
                    *chart_type = declared;
                    1
                }
                _ => 0,
            }
        }
        _ => 0,
    }
}

/// "Do", "Sol7", "lam", "am" -> "C", "G7", "Am", "Am": the dictionary is in
/// English letter names with a capital root.
fn english_chord(raw: &str) -> String {
    let t = raw.trim();
    let lower = t.to_lowercase();
    for (es, en) in [("sol", "G"), ("do", "C"), ("re", "D"), ("mi", "E"), ("fa", "F"), ("la", "A"), ("si", "B")] {
        if let Some(rest) = lower.strip_prefix(es) {
            let rest = rest.trim();
            let quality = if rest.starts_with('m') && !rest.starts_with("maj") { format!("m{}", &rest[1..]) } else { rest.to_string() };
            return format!("{en}{quality}");
        }
    }
    let mut chars = t.chars();
    match chars.next() {
        Some(c) if ('a'..='g').contains(&c.to_ascii_lowercase()) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
        _ => t.to_string(),
    }
}

fn repair_solid(features: &mut [crate::domain::notebook::SolidFeature], start_view: &mut String, units: &mut String) -> usize {
    use crate::domain::notebook::{SOLID_OPS, SOLID_PLANES, SOLID_PROFILES, SOLID_VIEWS};
    let mut fixes = snap(start_view, &SOLID_VIEWS, "iso");
    let unit = units.trim().to_lowercase();
    if !["mm", "cm", "m", "in"].contains(&unit.as_str()) {
        *units = "mm".to_string();
        fixes += 1;
    } else if unit != *units {
        *units = unit;
        fixes += 1;
    }
    repair_ids(features.iter_mut().map(|f| &mut f.id), &mut fixes);
    for f in features.iter_mut() {
        // Common synonyms first, then the catalogue default.
        let op = f.op.trim().to_lowercase();
        let op = match op.as_str() {
            "extrusion" | "extrusión" | "extruir" | "pad" | "boss" => "extrude".to_string(),
            "revolucion" | "revolución" | "revolver" | "lathe" => "revolve".to_string(),
            "pocket" | "corte" | "cortar" | "vaciado" | "subtract" => "cut".to_string(),
            "agujero" | "taladro" | "drill" => "hole".to_string(),
            _ => op,
        };
        if op != f.op {
            f.op = op;
            fixes += 1;
        }
        fixes += snap(&mut f.op, &SOLID_OPS, "extrude");
        fixes += snap(&mut f.plane, &SOLID_PLANES, "xy");
        fixes += snap(&mut f.profile.kind, &SOLID_PROFILES, "rect");
        // A square hole is a cut: same geometry, the name the frontend knows.
        if f.op == "hole" && f.profile.kind != "circle" {
            f.op = "cut".to_string();
            fixes += 1;
        }
        // A circle sized with `w` instead of its diameter `d`.
        if f.profile.kind == "circle" && f.profile.d.is_none() {
            if let Some(w) = f.profile.w {
                f.profile.d = Some(w);
                fixes += 1;
            }
        }
        // A revolve turns 360°: a depth there is the angle or a leftover, never a length.
        if f.op == "revolve" && f.depth.is_some() {
            f.depth = None;
            fixes += 1;
        }
    }
    fixes += repair_cut_direction(features);
    fixes
}

/// A cut or hole with a `depth` goes INTO the part from its sketch plane
/// (towards -normal). Models often sketch it on the floor (offset 0) and
/// mean "drill 8 mm up through the plate": as written it bites nothing.
/// When only the opposite direction reaches the material, that is what was
/// meant — move the sketch plane to the other end of the same range.
fn repair_cut_direction(features: &mut [crate::domain::notebook::SolidFeature]) -> usize {
    use crate::domain::solid_model::{feature_box, is_additive};
    let through = crate::domain::notebook::MAX_SOLID_DIM * 4.0;
    let mut fixes = 0;
    for i in 0..features.len() {
        let (before, rest) = features.split_at_mut(i);
        let f = &mut rest[0];
        let Some(depth) = f.depth.filter(|d| *d > 0.0) else { continue };
        if is_additive(&f.op) {
            continue;
        }
        let material: Vec<_> = before.iter().filter(|p| is_additive(&p.op)).filter_map(|p| feature_box(p, through)).collect();
        let bites = |f: &crate::domain::notebook::SolidFeature| {
            feature_box(f, through).is_some_and(|b| material.iter().any(|m| (0..3).all(|k| b.min[k] < m.max[k] - 1e-6 && m.min[k] < b.max[k] - 1e-6)))
        };
        if bites(f) {
            continue;
        }
        let mut flipped = f.clone();
        flipped.offset += depth;
        if bites(&flipped) {
            f.offset = flipped.offset;
            fixes += 1;
        }
    }
    fixes
}

// --- topology_flow / explorable_model ------------------------------------------

/// An off-catalogue word becomes the catalogue's default (case and dashes
/// are forgiven first): the picture is still right, only less specific.
fn snap(value: &mut String, catalogue: &[&str], default: &str) -> usize {
    let norm = value.trim().to_lowercase().replace(['-', ' '], "_");
    let fixed = if catalogue.contains(&norm.as_str()) { norm } else { default.to_string() };
    if fixed == *value {
        return 0;
    }
    *value = fixed;
    1
}

fn snap_opt(value: &mut Option<String>, catalogue: &[&str], default: &str) -> usize {
    match value {
        Some(v) => snap(v, catalogue, default),
        None => {
            *value = Some(default.to_string());
            0 // filling an omitted optional is not a "fix" worth reporting
        }
    }
}

/// Valid, unique ids; returns `original -> final` for the references to follow.
fn repair_ids<'a>(ids: impl Iterator<Item = &'a mut String>, fixes: &mut usize) -> (HashSet<String>, HashMap<String, String>) {
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut used: HashSet<String> = HashSet::new();
    for slot in ids {
        let original = slot.clone();
        let mut id = sanitize_id(&original);
        let base = id.clone();
        let mut n = 2;
        while used.contains(&id) {
            id = format!("{base}{n}");
            n += 1;
        }
        if id != original {
            *fixes += 1;
        }
        used.insert(id.clone());
        renamed.entry(original).or_insert_with(|| id.clone());
        *slot = id;
    }
    (used, renamed)
}

fn repair_topology(
    nodes: &mut [crate::domain::notebook::TopologyNode],
    links: &mut Vec<crate::domain::notebook::TopologyLink>,
    steps: &mut [crate::domain::notebook::FlowStep],
) -> usize {
    use crate::domain::notebook::{FLOW_TONES, TOPOLOGY_ICONS};
    let mut fixes = 0;
    let (used, renamed) = repair_ids(nodes.iter_mut().map(|n| &mut n.id), &mut fixes);
    let by_label: HashMap<String, String> = nodes.iter().map(|n| (n.label.trim().to_lowercase(), n.id.clone())).collect();
    let resolve = |end: &mut String, fixes: &mut usize| {
        if used.contains(end.as_str()) {
            return;
        }
        if let Some(id) = renamed.get(end.as_str()).or_else(|| by_label.get(&end.trim().to_lowercase())) {
            *end = id.clone();
            *fixes += 1;
        }
    };
    for node in nodes.iter_mut() {
        fixes += snap(&mut node.icon, &TOPOLOGY_ICONS, "box");
    }
    for link in links.iter_mut() {
        resolve(&mut link.from, &mut fixes);
        resolve(&mut link.to, &mut fixes);
    }
    // Self-links, links to nothing and repeated links (either direction) cannot be drawn.
    let before = links.len();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    links.retain(|l| {
        let key = if l.from <= l.to { (l.from.clone(), l.to.clone()) } else { (l.to.clone(), l.from.clone()) };
        l.from != l.to && used.contains(&l.from) && used.contains(&l.to) && seen.insert(key)
    });
    fixes += before - links.len();
    for step in steps.iter_mut() {
        for m in step.messages.iter_mut() {
            resolve(&mut m.from, &mut fixes);
            resolve(&mut m.to, &mut fixes);
            fixes += snap_opt(&mut m.tone, &FLOW_TONES, "request");
        }
    }
    fixes
}

fn repair_model(shell: Option<&mut crate::domain::notebook::ModelShell>, parts: &mut [crate::domain::notebook::ModelPart]) -> usize {
    use crate::domain::notebook::{MODEL_COLORS, MODEL_SHAPES, MODEL_SHELL_SHAPES, MODEL_SIZES, MODEL_ZONES};
    let mut fixes = 0;
    if let Some(shell) = shell {
        fixes += snap(&mut shell.shape, &MODEL_SHELL_SHAPES, "sphere");
    }
    repair_ids(parts.iter_mut().map(|p| &mut p.id), &mut fixes);
    // Only ONE thing fits at the centre (it is a point, not a band): the model
    // often puts the nucleus AND its contents there, which used to cost a whole
    // regeneration. Keep the innermost layer if there is one, else the first
    // centre part, and move the rest one band out.
    let centre = |p: &crate::domain::notebook::ModelPart| p.zone.as_deref().is_some_and(|z| z.trim().eq_ignore_ascii_case("center"));
    let keep = parts
        .iter()
        .position(|p| centre(p) && p.shape.trim().eq_ignore_ascii_case("layer"))
        .or_else(|| parts.iter().position(|p| centre(p)));
    for (i, part) in parts.iter_mut().enumerate() {
        if centre(part) && Some(i) != keep {
            part.zone = Some("inner".to_string());
            fixes += 1;
        }
    }
    for (i, part) in parts.iter_mut().enumerate() {
        fixes += snap(&mut part.shape, &MODEL_SHAPES, "sphere");
        fixes += snap_opt(&mut part.size, &MODEL_SIZES, "medium");
        fixes += snap_opt(&mut part.zone, &MODEL_ZONES, "inner");
        // An omitted colour cycles through the palette so parts stay distinguishable.
        let fallback = MODEL_COLORS[i % (MODEL_COLORS.len() - 1)];
        fixes += snap_opt(&mut part.color, &MODEL_COLORS, fallback);
        // A layer is one concentric shell: copies of it mean nothing.
        let clamped = if part.shape == "layer" { 1 } else { part.count.clamp(1, 12) };
        if clamped != part.count {
            part.count = clamped;
            fixes += 1;
        }
    }
    fixes
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
            visual_intent: None,
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
    fn topology_references_follow_repaired_ids_and_undrawable_links_go() {
        use crate::domain::notebook::{FlowMessage, FlowStep, TopologyLink, TopologyNode};
        let node = |id: &str, label: &str, icon: &str| TopologyNode { id: id.into(), label: label.into(), icon: icon.into(), subtext: None, explanation: None };
        let link = |from: &str, to: &str| TopologyLink { from: from.into(), to: to.into() };
        let mut v = StaticVisualSpec::TopologyFlow {
            title: None,
            nodes: vec![node("Mi PC", "Cliente", "Laptop"), node("srv", "Servidor web", "mainframe")],
            links: vec![link("Mi PC", "srv"), link("srv", "Mi PC"), link("srv", "srv"), link("srv", "ghost")],
            steps: vec![FlowStep {
                title: "t".into(),
                detail: "d".into(),
                messages: vec![FlowMessage { from: "Cliente".into(), to: "srv".into(), label: "GET /".into(), tone: Some("Reply".into()) }],
            }],
            caption: Some("c".into()),
        };
        assert!(repair_visual(&mut v) >= 6);
        let StaticVisualSpec::TopologyFlow { nodes, links, steps, .. } = &v else { panic!() };
        assert_eq!(nodes[0].id, "Mi_PC");
        assert_eq!((nodes[0].icon.as_str(), nodes[1].icon.as_str()), ("laptop", "box"), "case is forgiven, unknown icons fall back");
        assert_eq!(links.len(), 1, "duplicate, self and dangling links are dropped");
        assert_eq!((links[0].from.as_str(), links[0].to.as_str()), ("Mi_PC", "srv"));
        assert_eq!(steps[0].messages[0].from, "Mi_PC", "a label resolves to its id");
        assert_eq!(steps[0].messages[0].tone.as_deref(), Some("request"));
    }

    #[test]
    fn model_parts_snap_to_the_catalogue_and_a_clean_model_is_untouched() {
        use crate::domain::notebook::{ModelPart, ModelShell};
        let part = |id: &str, shape: &str, count: u32| ModelPart {
            id: id.into(),
            label: "L".into(),
            shape: shape.into(),
            size: Some("medium".into()),
            count,
            zone: Some("inner".into()),
            color: Some("sky".into()),
            explanation: "e".into(),
        };
        let mut v = StaticVisualSpec::ExplorableModel {
            title: None,
            shell: Some(ModelShell { label: "Membrana".into(), shape: "pyramid".into(), cutaway: true }),
            parts: vec![part("nucleo", "Disc-Stack", 40), part("nucleo", "dodecahedron", 1)],
            caption: Some("c".into()),
        };
        assert_eq!(repair_visual(&mut v), 5);
        let StaticVisualSpec::ExplorableModel { shell, parts, .. } = &v else { panic!() };
        assert_eq!(shell.as_ref().unwrap().shape, "sphere");
        assert_eq!((parts[0].shape.as_str(), parts[0].count), ("disc_stack", 12));
        assert_eq!((parts[1].id.as_str(), parts[1].shape.as_str()), ("nucleo2", "sphere"));
        assert_eq!(repair_visual(&mut v), 0, "repair is idempotent");
    }

    fn model_part(id: &str, label: &str, shape: &str, zone: &str) -> crate::domain::notebook::ModelPart {
        crate::domain::notebook::ModelPart {
            id: id.into(),
            label: label.into(),
            shape: shape.into(),
            size: Some("medium".into()),
            count: 1,
            zone: Some(zone.into()),
            color: Some("sky".into()),
            explanation: "e".into(),
        }
    }

    #[test]
    fn extra_centre_parts_move_out_and_the_innermost_layer_keeps_the_centre() {
        let mut v = StaticVisualSpec::ExplorableModel {
            title: None,
            shell: None,
            parts: vec![
                model_part("protones", "Protones", "grains", "center"),
                model_part("nucleo", "Núcleo interno", "layer", "center"),
                model_part("neutrones", "Neutrones", "grains", "center"),
            ],
            caption: Some("c".into()),
        };
        assert_eq!(repair_visual(&mut v), 2);
        let StaticVisualSpec::ExplorableModel { parts, .. } = &v else { panic!() };
        let zones: Vec<&str> = parts.iter().map(|p| p.zone.as_deref().unwrap()).collect();
        assert_eq!(zones, ["inner", "center", "inner"]);
        assert_eq!(repair_visual(&mut v), 0, "repair is idempotent");
    }

    #[test]
    fn a_layer_is_a_single_shell() {
        let mut layer = model_part("manto", "Manto", "Layer", "outer");
        layer.count = 4;
        let mut v = StaticVisualSpec::ExplorableModel { title: None, shell: None, parts: vec![layer, model_part("x", "X", "sphere", "inner")], caption: Some("c".into()) };
        repair_visual(&mut v);
        let StaticVisualSpec::ExplorableModel { parts, .. } = &v else { panic!() };
        assert_eq!((parts[0].shape.as_str(), parts[0].count), ("layer", 1));
    }

    #[test]
    fn walkthrough_targets_resolve_to_highlightable_ids() {
        use crate::domain::notebook::{FlowMessage, FlowStep, ModelShell, TopologyLink, TopologyNode, WalkthroughStep};
        let step = |t: &str| WalkthroughStep { step_number: 1, target_visual_element: t.into(), pedagogical_insight: "i".into() };
        let model = StaticVisualSpec::ExplorableModel {
            title: None,
            shell: Some(ModelShell { label: "Superficie de la Tierra".into(), shape: "sphere".into(), cutaway: true }),
            parts: vec![model_part("nucleo_externo", "Núcleo externo", "layer", "inner"), model_part("manto", "Manto", "layer", "outer")],
            caption: Some("c".into()),
        };
        let mut steps = vec![step("manto"), step("Núcleo Externo"), step("shell"), step("Superficie de la Tierra"), step("el manto terrestre"), step("corteza")];
        assert_eq!(repair_walkthrough(&model, &mut steps), 3);
        let got: Vec<&str> = steps.iter().map(|s| s.target_visual_element.as_str()).collect();
        assert_eq!(got, ["manto", "nucleo_externo", "shell", "shell", "manto", "corteza"], "unknown targets are left alone");

        let node = |id: &str| TopologyNode { id: id.into(), label: id.to_uppercase(), icon: "laptop".into(), subtext: None, explanation: None };
        let flow = StaticVisualSpec::TopologyFlow {
            title: None,
            nodes: vec![node("cliente"), node("servidor")],
            links: vec![TopologyLink { from: "cliente".into(), to: "servidor".into() }],
            steps: vec![FlowStep {
                title: "Primer saludo".into(),
                detail: "d".into(),
                messages: vec![FlowMessage { from: "cliente".into(), to: "servidor".into(), label: "SYN (seq=100)".into(), tone: None }],
            }],
            caption: Some("c".into()),
        };
        let mut steps = vec![step("SYN (seq=100)"), step("primer saludo"), step("CLIENTE")];
        assert_eq!(repair_walkthrough(&flow, &mut steps), 3);
        let got: Vec<&str> = steps.iter().map(|s| s.target_visual_element.as_str()).collect();
        assert_eq!(got, ["servidor", "servidor", "cliente"]);
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

#[cfg(test)]
mod solid_and_intent_tests {
    use super::*;
    use crate::domain::notebook::{MermaidChartType, SolidFeature, SolidProfile, WalkthroughStep};

    fn feature(id: &str, op: &str, kind: &str) -> SolidFeature {
        SolidFeature {
            id: id.into(),
            label: id.into(),
            op: op.into(),
            plane: "XY".into(),
            profile: SolidProfile { kind: kind.into(), w: Some(6.0), h: Some(6.0), d: None, points: vec![] },
            at: [0.0, 0.0],
            offset: 0.0,
            depth: Some(4.0),
            explanation: "e".into(),
        }
    }

    #[test]
    fn solid_features_are_snapped_and_walkthrough_targets_find_views() {
        let visual = StaticVisualSpec::SolidModel {
            title: None,
            units: "MM".into(),
            features: vec![feature("base plate", "Extrusión", "rectangle"), feature("hueco", "agujero", "rect"), feature("eje", "hole", "circle")],
            show_camera: false,
            start_view: "Isometric".into(),
            caption: None,
        };
        let step = |t: &str| WalkthroughStep { step_number: 1, target_visual_element: t.into(), pedagogical_insight: "i".into() };
        let mut block = GeneratedSectionBlock::DeclarativeVisualDiagram {
            title: "t".into(),
            visual_aid: visual,
            guided_walkthrough: vec![step("Vista frontal"), step("tu cámara"), step("base plate")],
            visual_intent: Some("Spatial Shape".into()),
        };
        assert!(repair_block(&mut block) > 0);
        let GeneratedSectionBlock::DeclarativeVisualDiagram {
            visual_aid: StaticVisualSpec::SolidModel { features, units, start_view, .. },
            guided_walkthrough,
            visual_intent,
            ..
        } = &block
        else {
            panic!()
        };
        assert_eq!((units.as_str(), start_view.as_str()), ("mm", "iso"));
        assert_eq!(features[0].id, "base_plate");
        assert_eq!((features[0].op.as_str(), features[0].plane.as_str(), features[0].profile.kind.as_str()), ("extrude", "xy", "rect"));
        assert_eq!(features[1].op, "cut", "a square hole is a cut");
        assert_eq!(features[2].profile.d, Some(6.0), "circle sized with w gets its diameter");
        let targets: Vec<&str> = guided_walkthrough.iter().map(|s| s.target_visual_element.as_str()).collect();
        assert_eq!(targets, ["front", "camera", "base_plate"]);
        assert_eq!(visual_intent.as_deref(), Some("spatial_shape"));
    }

    #[test]
    fn a_hole_drilled_from_the_floor_is_turned_into_the_part() {
        let rect = SolidProfile { kind: "rect".into(), w: Some(60.0), h: Some(40.0), d: None, points: vec![] };
        let circle = SolidProfile { kind: "circle".into(), w: None, h: None, d: Some(8.0), points: vec![] };
        let mut base = feature("base", "extrude", "rect");
        base.profile = rect;
        base.plane = "xy".into();
        base.depth = Some(8.0);
        let mut hole = feature("agujero", "hole", "circle");
        hole.profile = circle;
        hole.plane = "xy".into();
        hole.at = [15.0, 0.0];
        hole.depth = Some(8.0);
        let mut vase = feature("vaso", "revolve", "rect");
        vase.depth = Some(360.0);
        let mut features = vec![base, hole, vase];
        assert!(repair_solid(&mut features, &mut "iso".to_string(), &mut "mm".to_string()) >= 2);
        assert_eq!(features[1].offset, 8.0, "the sketch moves to the top face so the hole goes down through the plate");
        assert_eq!(features[2].depth, None, "a revolve has no depth");
        assert!(crate::domain::solid_model::solid_violations(0, &features[..2]).is_empty(), "{:?}", crate::domain::solid_model::solid_violations(0, &features[..2]));
    }

    #[test]
    fn spanish_chord_and_piece_names_are_translated() {
        assert_eq!(english_chord("Do"), "C");
        assert_eq!(english_chord("Sol7"), "G7");
        assert_eq!(english_chord("Lam"), "Am");
        assert_eq!(english_chord("am"), "Am");
        assert_eq!(english_chord("Fmaj7"), "Fmaj7");
        assert_eq!(english_chord("Remaj7"), "Dmaj7");
    }

    #[test]
    fn a_mermaid_chart_type_follows_its_header() {
        let mut v = StaticVisualSpec::Mermaid { chart_type: MermaidChartType::Flowchart, code: "mindmap\n  root((CAD))\n    Croquis".into(), caption: "c".into() };
        assert_eq!(repair_visual(&mut v), 1);
        assert!(matches!(v, StaticVisualSpec::Mermaid { chart_type: MermaidChartType::Mindmap, .. }));
    }
}
