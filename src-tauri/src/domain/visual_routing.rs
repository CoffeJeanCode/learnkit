//! Which drawing fits which content — the "intelligent" part of the diagram
//! generator, split so each side does what it is good at:
//!
//! - The MODEL judges the content: for every figure it names a
//!   `visualIntent` (a spatial shape, a process, a hierarchy, a chronology…)
//!   before choosing the engine. That judgement is per concept, not per
//!   course: a CAD class about the design workflow is a process, the same
//!   class's part is a shape.
//! - The SYSTEM checks coherence, deterministically and for free: the engine
//!   must be one that draws that intent ([`VISUAL_INTENTS`]), and an example
//!   that is unmistakably spatial (camera orbiting a part, standard views,
//!   extrusions…) is never accepted as boxes and arrows ([`spatial_signal`]).
//! - The COURSE only gives a hint ([`classify`] → [`directive`]): what kind
//!   of figure usually fits, and a lower bar for "this text is spatial".

use crate::domain::notebook::{InteractiveModuleSpec, StaticVisualSpec};

/// One kind of content and the engines (`engine` or `engine:subtype`) that draw it.
pub struct VisualIntent {
    pub key: &'static str,
    /// What content it is, for the prompt and the tool schema.
    pub describes: &'static str,
    pub engines: &'static [&'static str],
}

pub const VISUAL_INTENTS: &[VisualIntent] = &[
    VisualIntent {
        key: "spatial_shape",
        describes: "una FORMA en el espacio: pieza con medidas, sólido, vistas, ejes, planos, la cámara que la orbita",
        engines: &["solid_model", "explorable_model", "declarative_svg"],
    },
    VisualIntent {
        key: "object_parts",
        describes: "un objeto con partes que se entiende girándolo (célula, átomo, motor, planeta por capas)",
        engines: &["explorable_model", "solid_model", "declarative_svg"],
    },
    VisualIntent {
        key: "curved_surface",
        describes: "una SUPERFICIE o un CAMPO: espacio-tiempo curvo, pozos de potencial, z = f(x, y), volumen bajo una superficie, \
                    trayectorias (geodésicas, descensos) sobre ella",
        engines: &["surface_3d"],
    },
    VisualIntent {
        key: "molecule",
        describes: "una MOLÉCULA o una reacción: su estructura en 3D y el mecanismo paso a paso (enlaces que se forman y se rompen)",
        engines: &["molecule_3d"],
    },
    VisualIntent {
        key: "algebraic",
        describes: "una ESTRUCTURA ALGEBRAICA finita: un grupo, su tabla de Cayley, un subgrupo y sus clases laterales",
        engines: &["algebraic_structure"],
    },
    VisualIntent {
        key: "music",
        describes: "MÚSICA en un instrumento: acordes, una progresión, un rasgueo o un ritmo que se ve y se escucha",
        engines: &["instrument"],
    },
    VisualIntent {
        key: "accumulation",
        describes: "un área que se ACUMULA bajo una curva (la integral), sumas de Riemann, la tangente que da la derivada",
        engines: &["interactive_module:function_plot"],
    },
    VisualIntent {
        key: "physical_phenomenon",
        describes: "un fenómeno físico o biológico en el espacio (membrana, campo, fuerzas, difusión)",
        engines: &["illustration_kit", "declarative_svg", "explorable_model", "interactive_module:particle_gradient", "interactive_module:vector_sum"],
    },
    VisualIntent {
        key: "law",
        describes: "una ley o fórmula cuyas variables cambian un resultado",
        engines: &["interactive_module:relation_explorer", "interactive_module:function_plot", "interactive_module:vector_sum"],
    },
    VisualIntent {
        key: "read_in_pieces",
        describes: "algo que se lee por trozos en orden y su variante (codones, bits, octetos, partes de una URL)",
        engines: &["sequence_strip"],
    },
    VisualIntent {
        key: "messages",
        describes: "mensajes que viajan entre equipos o actores, paso a paso",
        engines: &["topology_flow", "mermaid:sequenceDiagram", "diagram_spec:sequence"],
    },
    VisualIntent {
        key: "process",
        describes: "un proceso o flujo con pasos y decisiones",
        engines: &["diagram_spec:flowchart", "diagram_spec:cycle", "mermaid:flowchart", "interactive_module:timeline"],
    },
    VisualIntent {
        key: "states",
        describes: "estados de algo y las transiciones entre ellos",
        engines: &["diagram_spec:state", "mermaid:stateDiagram"],
    },
    VisualIntent {
        key: "hierarchy",
        describes: "una jerarquía de conceptos alrededor de una idea raíz",
        engines: &["mermaid:mindmap"],
    },
    VisualIntent {
        key: "chronology",
        describes: "hechos fechados en orden (historia, versiones, etapas de una época)",
        engines: &["mermaid:timeline", "interactive_module:timeline"],
    },
    VisualIntent { key: "schedule", describes: "un plan de trabajo en el calendario", engines: &["mermaid:gantt"] },
    VisualIntent { key: "proportion", describes: "partes de un todo en porcentajes", engines: &["mermaid:pie"] },
    VisualIntent {
        key: "two_axes",
        describes: "elementos clasificados en dos ejes (impacto × esfuerzo, riesgo × probabilidad)",
        engines: &["mermaid:quadrantChart", "conceptual_matrix"],
    },
    VisualIntent {
        key: "data_trend",
        describes: "datos numéricos por categoría o en el tiempo (barras, línea)",
        engines: &["mermaid:xychart", "interactive_module:function_plot"],
    },
    VisualIntent { key: "quantity_flow", describes: "cantidades que fluyen y se reparten (energía, presupuesto, tráfico)", engines: &["mermaid:sankey"] },
    VisualIntent { key: "entities", describes: "entidades o clases y sus relaciones", engines: &["mermaid:classDiagram", "mermaid:erDiagram"] },
    VisualIntent { key: "comparison", describes: "una comparación o taxonomía en filas y columnas", engines: &["conceptual_matrix", "diagram_spec:compare"] },
    VisualIntent { key: "branches", describes: "ramas e integraciones de un historial (git)", engines: &["mermaid:gitGraph"] },
    VisualIntent { key: "experience", describes: "la experiencia de alguien etapa por etapa", engines: &["mermaid:journey"] },
];

/// Intents whose content lives in space: only 3D or an illustration draws them.
const SPATIAL_INTENTS: [&str; 4] = ["spatial_shape", "object_parts", "physical_phenomenon", "curved_surface"];
const SPATIAL_ENGINES: [&str; 7] = ["solid_model", "explorable_model", "declarative_svg", "surface_3d", "molecule_3d", "illustration_kit", "code_figure"];
/// Words that only a CURVED space or a field uses: no formula slider, box or
/// table shows them — only a surface the student turns around does.
const CURVATURE_TERMS: &[&str] = &[
    "curvatura", "geodesic*", "pozo de potencial", "pozo gravitatorio", "espacio tiempo curvo", "curva el espacio", "superficie curva",
    "metrica del espacio",
];

pub fn intent(key: &str) -> Option<&'static VisualIntent> {
    VISUAL_INTENTS.iter().find(|i| i.key == key)
}

/// Snaps what the model wrote ("Spatial Shape", "3d", "proceso") to an intent
/// key; `None` when nothing matches unambiguously.
pub fn normalize_intent(raw: &str) -> Option<&'static str> {
    let k = raw.trim().to_lowercase().replace(['-', ' '], "_");
    if let Some(i) = intent(&k) {
        return Some(i.key);
    }
    let alias = match k.as_str() {
        "spatial" | "shape" | "3d" | "solid" | "part" | "forma" | "espacial" | "pieza" => "spatial_shape",
        "object" | "objeto" | "parts" => "object_parts",
        "phenomenon" | "fenomeno" | "physical" => "physical_phenomenon",
        "formula" | "relation" | "ley" => "law",
        "sequence" | "secuencia" | "pieces" => "read_in_pieces",
        "message" | "network" | "mensajes" | "protocol" => "messages",
        "flow" | "flowchart" | "proceso" | "steps" => "process",
        "state" | "estados" | "state_machine" => "states",
        "surface" | "superficie" | "field" | "campo" | "curvature" | "curvatura" => "curved_surface",
        "integral" | "area" | "accumulation" | "acumulacion" => "accumulation",
        "chemistry" | "quimica" | "molecula" | "reaction" | "reaccion" | "mechanism" | "mecanismo" => "molecule",
        "group" | "grupo" | "algebra" | "group_theory" => "algebraic",
        "musica" | "chords" | "acordes" | "rhythm" | "ritmo" | "guitar" | "guitarra" => "music",
        "mindmap" | "jerarquia" | "tree" | "concept_map" => "hierarchy",
        "timeline" | "history" | "cronologia" | "historia" => "chronology",
        "gantt" | "plan" | "planning" => "schedule",
        "pie" | "share" | "proporciones" => "proportion",
        "quadrant" | "matrix_2x2" | "cuadrante" => "two_axes",
        "chart" | "trend" | "data" | "xychart" => "data_trend",
        "sankey" | "flujo_de_cantidad" => "quantity_flow",
        "class" | "er" | "entity" | "entidades" => "entities",
        "compare" | "comparacion" | "table" => "comparison",
        "git" | "gitgraph" => "branches",
        "journey" | "experiencia" => "experience",
        _ => return None,
    };
    Some(alias)
}

/// `engine` and, where the engine has subtypes, `engine:subtype`.
pub fn engine_keys(visual: &StaticVisualSpec) -> (&'static str, Option<String>) {
    match visual {
        StaticVisualSpec::Mermaid { chart_type, .. } => ("mermaid", Some(format!("mermaid:{}", chart_type.as_str()))),
        StaticVisualSpec::DiagramSpec { kind, .. } => ("diagram_spec", Some(format!("diagram_spec:{kind}"))),
        StaticVisualSpec::InteractiveModule { spec, .. } => {
            let m = match spec {
                InteractiveModuleSpec::RelationExplorer { .. } => "relation_explorer",
                InteractiveModuleSpec::FunctionPlot { .. } => "function_plot",
                InteractiveModuleSpec::VectorSum { .. } => "vector_sum",
                InteractiveModuleSpec::ParticleGradient { .. } => "particle_gradient",
                InteractiveModuleSpec::Timeline { .. } => "timeline",
            };
            ("interactive_module", Some(format!("interactive_module:{m}")))
        }
        StaticVisualSpec::DeclarativeSvg { .. } => ("declarative_svg", None),
        StaticVisualSpec::SequenceStrip { .. } => ("sequence_strip", None),
        StaticVisualSpec::ConceptualMatrix { .. } => ("conceptual_matrix", None),
        StaticVisualSpec::TopologyFlow { .. } => ("topology_flow", None),
        StaticVisualSpec::ExplorableModel { .. } => ("explorable_model", None),
        StaticVisualSpec::SolidModel { .. } => ("solid_model", None),
        StaticVisualSpec::Surface3d { .. } => ("surface_3d", None),
        StaticVisualSpec::Molecule3d { .. } => ("molecule_3d", None),
        StaticVisualSpec::AlgebraicStructure { .. } => ("algebraic_structure", None),
        StaticVisualSpec::Instrument { .. } => ("instrument", None),
        StaticVisualSpec::IllustrationKit { .. } => ("illustration_kit", None),
        StaticVisualSpec::CodeFigure { .. } => ("code_figure", None),
    }
}

pub fn fits(intent: &VisualIntent, visual: &StaticVisualSpec) -> bool {
    let (engine, sub) = engine_keys(visual);
    // Tier 2 draws anything: it is gated by WHO may write it, not by intent.
    if engine == "code_figure" {
        return true;
    }
    intent.engines.iter().any(|e| *e == engine || Some(*e) == sub.as_deref())
}

fn engine_label(visual: &StaticVisualSpec) -> String {
    let (engine, sub) = engine_keys(visual);
    sub.unwrap_or_else(|| engine.to_string())
}

// --- course hint -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualDomain {
    /// CAD, technical drawing, 3D modelling/printing, mechanical design.
    Spatial3d,
    /// Networks, protocols, distributed systems.
    Network,
    /// Physics, calculus, statistics, economics: laws with variables.
    Quantitative,
    General,
}

/// Lower-case, accents folded, words kept (unlike `visual_repair::fold`).
fn normalize(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect()
}

/// Does `term` occur as a word (or, with a trailing `*`, as a word prefix)?
fn has_term(text: &str, term: &str) -> bool {
    let (stem, prefix) = match term.strip_suffix('*') {
        Some(s) => (s, true),
        None => (term, false),
    };
    let padded = format!(" {text} ");
    let mut from = 0;
    while let Some(pos) = padded[from..].find(stem) {
        let at = from + pos;
        let before_ok = padded[..at].ends_with(' ');
        let after = &padded[at + stem.len()..];
        if before_ok && (prefix || after.starts_with(' ')) {
            return true;
        }
        from = at + stem.len().max(1);
    }
    false
}

fn count_terms(text: &str, terms: &[&str]) -> usize {
    terms.iter().filter(|t| has_term(text, t)).count()
}

const SPATIAL_COURSE_TERMS: &[&str] = &[
    "cad", "autocad", "solidworks", "fusion 360", "fusion360", "freecad", "onshape", "inventor", "catia", "sketchup", "blender", "rhino",
    "modelado 3d", "modelado parametrico", "diseno 3d", "dibujo tecnico", "geometria descriptiva", "impresion 3d", "diseno mecanico",
    "diseno industrial", "planos tecnicos", "modelado de piezas", "3d",
];
const NETWORK_COURSE_TERMS: &[&str] =
    &["redes", "red", "tcp", "http", "https", "dns", "protocolo*", "microservicio*", "cliente servidor", "api rest", "sistemas distribuidos", "kubernetes"];
const QUANT_COURSE_TERMS: &[&str] =
    &["fisica", "calculo", "algebra", "estadistica", "probabilidad", "economia", "finanzas", "termodinamica", "mecanica clasica", "electricidad"];

/// What kind of course this is — a hint only (see the module docs).
pub fn classify(texts: &[&str]) -> VisualDomain {
    let text = normalize(&texts.join(" "));
    if count_terms(&text, SPATIAL_COURSE_TERMS) > 0 {
        VisualDomain::Spatial3d
    } else if count_terms(&text, NETWORK_COURSE_TERMS) > 0 {
        VisualDomain::Network
    } else if count_terms(&text, QUANT_COURSE_TERMS) > 0 {
        VisualDomain::Quantitative
    } else {
        VisualDomain::General
    }
}

/// The hint handed to the block generator (`visualDirective`).
pub fn directive(domain: VisualDomain) -> Option<&'static str> {
    match domain {
        VisualDomain::Spatial3d => Some(
            "Curso ESPACIAL (CAD / modelado 3D): cuando el ejemplo es una forma, una pieza, sus vistas, sus ejes o la cámara, \
             visualIntent = spatial_shape y la figura es `solid_model` (pieza con medidas reales; showCamera si el concepto es \
             la cámara o el punto de vista). Los procesos de trabajo (pasos del diseño) siguen siendo process.",
        ),
        VisualDomain::Network => Some(
            "Curso de REDES/SISTEMAS: lo que viaja entre equipos es messages (`topology_flow`); la estructura de un paquete o \
             una dirección es read_in_pieces (`sequence_strip`).",
        ),
        VisualDomain::Quantitative => Some(
            "Curso CUANTITATIVO: una ley con variables es law (`interactive_module` relation_explorer o function_plot), que el \
             estudiante mueve; datos medidos son data_trend.",
        ),
        VisualDomain::General => None,
    }
}

// --- spatial content ---------------------------------------------------------

/// Words that only a spatial example uses.
const SPATIAL_TERMS: &[&str] = &[
    "isometric*", "ortogonal*", "ortograf*", "extrus*", "extruir", "extruye*", "croquis", "boceto*", "eje x", "eje y", "eje z", "ejes x",
    "plano xy", "plano xz", "plano yz", "orbit*", "vista frontal", "vista superior", "vista lateral", "vista de planta", "seccion transversal",
    "corte transversal", "solido*", "agujero*", "chaflan*", "redondeo*", "ensamblaje*", "pieza*", "modelo 3d", "3d", "revolucion de un perfil",
    "cara superior", "arista*", "vertice*",
];
/// Concepts ABOUT looking at a shape: never a flowchart, whatever the intent.
const VIEW_TERMS: &[&str] = &[
    "vista isometrica", "vista frontal", "vista superior", "vista lateral", "proyeccion ortogonal", "proyecciones ortogonales", "cubo de vistas",
    "viewcube", "punto de vista",
];

/// Is this example unmistakably about something seen in space? In a spatial
/// course one spatial word is enough; elsewhere it takes two.
pub fn spatial_signal(text: &str, domain: VisualDomain) -> bool {
    let t = normalize(text);
    let n = count_terms(&t, SPATIAL_TERMS);
    n >= 2 || (n >= 1 && domain == VisualDomain::Spatial3d) || view_concept(&t)
}

fn view_concept(normalized: &str) -> bool {
    count_terms(normalized, VIEW_TERMS) > 0
        || (has_term(normalized, "camara") && (has_term(normalized, "orbit*") || has_term(normalized, "gira*") || has_term(normalized, "vista*")))
}

/// Engine/intent coherence for the figure of a theory or diagram block.
/// `text` is what the block says around the figure (title, rule, walkthrough).
pub fn routing_violations(i: usize, intent_key: Option<&str>, visual: &StaticVisualSpec, text: &str, domain: VisualDomain) -> Vec<String> {
    let mut v = Vec::new();
    let declared = intent_key.and_then(intent);
    let drawn = engine_label(visual);
    if let Some(intent) = declared {
        if !fits(intent, visual) {
            v.push(format!(
                "bloque {i}: visualIntent \"{}\" ({}) se dibuja con {} — elegiste {drawn}. Cambia el motor, o el visualIntent si \
                 el contenido es de otro tipo",
                intent.key,
                intent.describes,
                intent.engines.join(" | ")
            ));
        }
    }
    let (engine, _) = engine_keys(visual);
    let spatial_engine = SPATIAL_ENGINES.contains(&engine);
    let claims_spatial = declared.is_none_or(|d| SPATIAL_INTENTS.contains(&d.key));
    // A camera or a view drawn as boxes and arrows is always wrong, whatever
    // the declared intent; another deliberate choice (a timeline, a strip…)
    // that merely MENTIONS a view is the model's call.
    let boxes = matches!(visual, StaticVisualSpec::DiagramSpec { .. })
        || matches!(visual, StaticVisualSpec::Mermaid { chart_type, .. } if matches!(chart_type, crate::domain::notebook::MermaidChartType::Flowchart | crate::domain::notebook::MermaidChartType::StateDiagram | crate::domain::notebook::MermaidChartType::SequenceDiagram));
    let looks_at_shape = boxes && view_concept(&normalize(text));
    // Curvature is never a calculator or a box diagram, whatever the declared
    // intent (the real failure: "la gravedad es geometría" drawn as t = √(2h/g)).
    let curved = count_terms(&normalize(text), CURVATURE_TERMS) > 0;
    if curved && !matches!(engine, "surface_3d" | "declarative_svg" | "code_figure") {
        v.push(format!(
            "bloque {i}: el concepto es una CURVATURA o un campo (curvatura, geodésicas, pozos) y lo dibujaste con {drawn}, que no \
             muestra ninguna superficie. Usa visualIntent curved_surface con `surface_3d`: la superficie (p. ej. un pozo \
             -m / sqrt(x^2 + y^2 + 0.5) con un slider de masa) y trayectorias `geodesic` que el sistema traza sobre ella"
        ));
        return v;
    }
    if !spatial_engine && (looks_at_shape || (claims_spatial && spatial_signal(text, domain))) {
        v.push(format!(
            "bloque {i}: el ejemplo es ESPACIAL (una forma, sus vistas, sus ejes o la cámara que la mira) y lo dibujaste con {drawn} — \
             cajas y flechas no muestran el espacio. Usa visualIntent spatial_shape con `solid_model` (la pieza con medidas reales; \
             `showCamera: true` si el concepto es la cámara o el punto de vista) o `explorable_model` si es un objeto con partes"
        ));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::{DiagramNodeSpec, MermaidChartType};

    fn flow() -> StaticVisualSpec {
        StaticVisualSpec::DiagramSpec {
            kind: "flowchart".into(),
            direction: None,
            title: None,
            nodes: vec![DiagramNodeSpec { id: "a".into(), label: "Tu cámara".into(), shape: None, subtext: None, explanation: None }],
            edges: vec![],
            caption: None,
        }
    }

    fn mindmap() -> StaticVisualSpec {
        StaticVisualSpec::Mermaid { chart_type: MermaidChartType::Mindmap, code: "mindmap\n  root((CAD))".into(), caption: "c".into() }
    }

    fn solid() -> StaticVisualSpec {
        StaticVisualSpec::SolidModel { title: None, units: "mm".into(), features: vec![], show_camera: true, start_view: "iso".into(), caption: None }
    }

    #[test]
    fn courses_are_classified_by_whole_words() {
        assert_eq!(classify(&["Diseño de piezas en Fusion 360", ""]), VisualDomain::Spatial3d);
        assert_eq!(classify(&["Introducción al CAD"]), VisualDomain::Spatial3d);
        assert_eq!(classify(&["Cadenas de Markov"]), VisualDomain::General, "cad inside cadenas is no CAD");
        assert_eq!(classify(&["Biología celular"]), VisualDomain::General);
        assert_eq!(classify(&["Redes TCP/IP"]), VisualDomain::Network);
    }

    #[test]
    fn the_camera_orbit_is_never_a_flowchart() {
        let text = "Orbitar mueve tu cámara alrededor de la pieza; la pieza no se mueve.";
        // Even when the model calls it a process, a camera looking at a shape is spatial.
        let v = routing_violations(0, Some("process"), &flow(), text, VisualDomain::General);
        assert!(v.iter().any(|s| s.contains("ESPACIAL")), "{v:?}");
        assert!(routing_violations(0, Some("spatial_shape"), &solid(), text, VisualDomain::Spatial3d).is_empty());
    }

    #[test]
    fn the_declared_intent_must_fit_the_engine() {
        let v = routing_violations(0, Some("hierarchy"), &flow(), "Los comandos de CAD se agrupan por familia.", VisualDomain::General);
        assert!(v.iter().any(|s| s.contains("mermaid:mindmap")), "{v:?}");
        assert!(routing_violations(0, Some("hierarchy"), &mindmap(), "Los comandos se agrupan por familia.", VisualDomain::General).is_empty());
    }

    #[test]
    fn a_process_in_a_spatial_course_stays_a_process() {
        // The design workflow of a CAD course is a process: the model said so and nothing in it looks at a shape.
        let text = "Primero defines el requisito, luego eliges el material y al final validas con el cliente.";
        assert!(routing_violations(0, Some("process"), &flow(), text, VisualDomain::Spatial3d).is_empty());
        // Without a declared intent, a spatial course's spatial word is enough to ask for 3D.
        let extrude = "Dibujas el croquis y lo conviertes en volumen.";
        assert!(!routing_violations(0, None, &flow(), extrude, VisualDomain::Spatial3d).is_empty());
        assert!(routing_violations(0, None, &flow(), extrude, VisualDomain::General).is_empty(), "one word outside a spatial course is not enough");
    }

    #[test]
    fn a_view_merely_mentioned_by_another_deliberate_figure_is_left_alone() {
        let strip = StaticVisualSpec::SequenceStrip { title: None, rows: vec![], caption: "c".into() };
        let text = "La misma geometría sirve para una vista frontal o un corte.";
        assert!(routing_violations(0, Some("read_in_pieces"), &strip, text, VisualDomain::Spatial3d).is_empty());
        assert!(!routing_violations(0, Some("process"), &flow(), text, VisualDomain::Spatial3d).is_empty(), "boxes for a view still go back");
    }

    #[test]
    fn curvature_is_never_a_calculator() {
        use crate::domain::notebook::{InteractiveModuleSpec, ModuleOutput, ModuleVariable};
        let calc = StaticVisualSpec::InteractiveModule {
            spec: InteractiveModuleSpec::RelationExplorer {
                output: ModuleOutput { sym: "t".into(), label: "Tiempo".into(), unit: None, expr: "(2*h/g)^0.5".into() },
                variables: vec![ModuleVariable { sym: "h".into(), label: "Altura".into(), unit: None, min: 0.5, max: 5.0, step: None, init: 1.0 }],
            },
            challenge: None,
            caption: None,
        };
        // The real theory: declared `law`, which fits a calculator — the curvature still sends it back.
        let rule = "La gravedad no es una fuerza: es la forma del espacio-tiempo. Los cuerpos libres siguen geodésicas.";
        let v = routing_violations(0, Some("law"), &calc, rule, VisualDomain::Quantitative);
        assert!(v.iter().any(|s| s.contains("surface_3d")), "{v:?}");
        // A law that only mentions time dilation keeps its calculator.
        assert!(routing_violations(0, Some("law"), &calc, "El tiempo se dilata con la velocidad.", VisualDomain::Quantitative).is_empty());
    }

    #[test]
    fn intents_are_normalized() {
        assert_eq!(normalize_intent("Spatial Shape"), Some("spatial_shape"));
        assert_eq!(normalize_intent("3d"), Some("spatial_shape"));
        assert_eq!(normalize_intent("cronologia"), Some("chronology"));
        assert_eq!(normalize_intent("banana"), None);
    }

    #[test]
    fn every_intent_names_real_engines() {
        let engines = [
            "solid_model", "surface_3d", "explorable_model", "declarative_svg", "sequence_strip", "topology_flow", "conceptual_matrix",
            "molecule_3d", "algebraic_structure", "instrument", "illustration_kit", "code_figure",
        ];
        for i in VISUAL_INTENTS {
            for e in i.engines {
                let ok = engines.contains(e)
                    || e.strip_prefix("mermaid:").is_some_and(|t| MermaidChartType::ALL.iter().any(|c| c.as_str() == t))
                    || e.strip_prefix("diagram_spec:").is_some_and(|k| ["flowchart", "sequence", "state", "cycle", "compare"].contains(&k))
                    || e.strip_prefix("interactive_module:").is_some_and(|m| {
                        ["relation_explorer", "function_plot", "vector_sum", "particle_gradient", "timeline"].contains(&m)
                    });
                assert!(ok, "{} -> {e}", i.key);
            }
        }
    }
}
