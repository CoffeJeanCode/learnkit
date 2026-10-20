use serde::{Deserialize, Serialize};

use crate::domain::notebook::DynamicBlockType;

/// The ~5 epistemological/disciplinary profiles `agents::block_generator_agent`'s
/// system prompt ("COMPOSICIÓN DINÁMICA" section) describes in prose to pick a
/// STARTING disposition for a class's block sequence — never a rigid mold (a
/// class can, and often does, legitimately deviate from its profile's
/// suggested chain). This enum and `suggested_sequence` exist purely to make
/// that same information ALSO exist as inspectable, unit-testable data: before
/// this module, the only place the profile-to-sequence mapping lived was as
/// natural-language text inside the LLM prompt, which meant nobody could write
/// a test asserting e.g. "the debugging profile suggests `heuristic_error_audit`
/// before `hands_on_mission`". This table does not replace the prompt prose
/// (the model still needs the natural-language version) and is not wired into
/// any runtime rejection/guardrail — it's a parallel, testable source of truth
/// for what the prompt already says.
///
/// It IS wired into generation as a hint, though: `infer_profile` +
/// `suggest_next_block` feed `notebook_service::render` a `compositionHint`
/// (profile, chain, and the block that belongs at the next position), so the
/// generator stops defaulting to the same template for every class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisciplineProfile {
    /// "Fenómenos espaciales o biológicos" (anatomía, mecánica, campos,
    /// procesos naturales con estructura física real).
    SpatialBiological,
    /// "Razonamiento cuantitativo, cálculo u optimización".
    QuantitativeMath,
    /// "Procesos cíclicos, balances o flujos de sistemas".
    CyclicSystemic,
    /// "Arquitectura de software, datos o depuración".
    SoftwareDebugging,
    /// "Liderazgo, toma de decisiones o ciencias sociales" (el contenido ES
    /// una decisión con consecuencias, no un concepto o procedimiento
    /// técnico).
    LeadershipDecisions,
}

impl DisciplineProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SpatialBiological => "spatial_biological",
            Self::QuantitativeMath => "quantitative_math",
            Self::CyclicSystemic => "cyclic_systemic",
            Self::SoftwareDebugging => "software_debugging",
            Self::LeadershipDecisions => "leadership_decisions",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "spatial_biological" => Some(Self::SpatialBiological),
            "quantitative_math" => Some(Self::QuantitativeMath),
            "cyclic_systemic" => Some(Self::CyclicSystemic),
            "software_debugging" => Some(Self::SoftwareDebugging),
            "leadership_decisions" => Some(Self::LeadershipDecisions),
            _ => None,
        }
    }

    /// All 5 profiles, in the same order the prompt lists them — used by
    /// tests that want to sweep every profile rather than name them one by
    /// one.
    pub const ALL: [DisciplineProfile; 5] = [
        Self::SpatialBiological,
        Self::QuantitativeMath,
        Self::CyclicSystemic,
        Self::SoftwareDebugging,
        Self::LeadershipDecisions,
    ];
}

/// The suggested block chain for `profile`, transcribed verbatim from the
/// "COMPOSICIÓN DINÁMICA" section of `agents::block_generator_agent`'s system
/// prompt. This is a STARTING disposition, not a mold: the prompt explicitly
/// tells the model these are "puntos de partida, no una tabla que copias", so
/// nothing in this crate treats a deviation from this sequence as invalid.
///
/// Two things the prompt itself calls out that this table intentionally does
/// NOT capture as static data, to avoid silently changing behavior or
/// encoding a contradiction:
/// - `SoftwareDebugging`'s prose also allows an OPTIONAL second
///   `hands_on_mission` ("transferencia lejana") before the closing block,
///   which this table omits — the base 4-block chain below is the sequence
///   actually asserted by this module's tests.
/// - `QuantitativeMath`'s prose does not explicitly mention
///   `metacognitive_closure` in its chain (every other profile's prose does),
///   even though the prompt separately states the closing block is mandatory
///   for every class once mastery is reached. This table transcribes the
///   chain exactly as written rather than inventing the omitted step.
pub fn suggested_sequence(profile: DisciplineProfile) -> &'static [DynamicBlockType] {
    use DynamicBlockType::*;

    match profile {
        DisciplineProfile::SpatialBiological => &[
            AnchoredMicroTheory,
            DeclarativeVisualDiagram,
            InteractivePredictionGate,
            DeclarativeVisualDiagram,
            MetacognitiveClosure,
        ],
        DisciplineProfile::QuantitativeMath => {
            &[AnchoredMicroTheory, InteractivePredictionGate, HeuristicErrorAudit, HandsOnMission]
        }
        DisciplineProfile::CyclicSystemic => {
            &[AnchoredMicroTheory, DeclarativeVisualDiagram, HeuristicErrorAudit, MetacognitiveClosure]
        }
        DisciplineProfile::SoftwareDebugging => {
            &[HeuristicErrorAudit, InteractivePredictionGate, HandsOnMission, MetacognitiveClosure]
        }
        DisciplineProfile::LeadershipDecisions => {
            &[AnchoredMicroTheory, BranchingScenarioChallenge, HandsOnMission, MetacognitiveClosure]
        }
    }
}

/// Keyword table for [`infer_profile`]: lowercase stems, matched as substrings.
const PROFILE_KEYWORDS: [(DisciplineProfile, &[&str]); 5] = [
    (
        DisciplineProfile::SoftwareDebugging,
        &[
            "programa", "código", "codigo", "software", "rust", "python", "javascript", "typescript", "api", "bug", "depur", "algoritm",
            "base de datos", "sql", "react", "backend", "frontend", "compil", "función", "funcion", "git", "docker", "estructura de datos",
        ],
    ),
    (
        DisciplineProfile::QuantitativeMath,
        &[
            "cálculo", "calculo", "álgebra", "algebra", "ecuaci", "derivad", "integral", "matemát", "matemat", "estadíst", "estadist",
            "probabilidad", "optimiz", "fórmula", "formula", "geometr", "aritmétic", "aritmetic", "trigonom", "límite", "limite",
        ],
    ),
    (
        DisciplineProfile::CyclicSystemic,
        &["ciclo", "cíclic", "ciclic", "balance", "flujo", "retroaliment", "ecosistema", "cadena de suministro", "homeostasis", "sistémic", "sistemic", "inventario"],
    ),
    (
        DisciplineProfile::SpatialBiological,
        &[
            "anatom", "biolog", "célula", "celula", "fisiolog", "física", "fisica", "mecánica", "mecanica", "campo", "química", "quimica",
            "órgano", "organo", "neurona", "molécula", "molecula", "geolog", "astronom", "óptica", "optica",
        ],
    ),
    (
        DisciplineProfile::LeadershipDecisions,
        &[
            "liderazgo", "líder", "lider", "gestión", "gestion", "equipo", "negociaci", "estrategia", "decisi", "ética", "etica", "management",
            "comunicaci", "sociolog", "política", "politica", "historia", "conflicto",
        ],
    ),
];

/// Best-effort guess of the class's [`DisciplineProfile`] from its own text
/// (the class title first, then milestone / course titles and goal).
/// Deterministic and deliberately dumb: it only seeds a hint the generator may
/// override in its `pedagogicalRationale`. `None` when no keyword matches, in
/// which case the generator falls back to the prose in its system prompt.
pub fn infer_profile(class_title: &str, other_context: &[&str]) -> Option<DisciplineProfile> {
    let class_lower = class_title.to_lowercase();
    let other_lower = other_context.join(" ").to_lowercase();
    // Stems match at WORD starts (so `api` never fires inside `capacidad`);
    // multi-word stems match as phrases.
    let mentions = |text: &str, stem: &str| {
        if stem.contains(' ') {
            text.contains(stem)
        } else {
            text.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(stem))
        }
    };
    // The class title is decisive: a course about code can still have a day
    // on derivatives. Course/milestone text only breaks ties between profiles
    // the title matches equally, or decides when the title matches nothing.
    let mut best: Option<(DisciplineProfile, (usize, usize))> = None;
    for (profile, stems) in PROFILE_KEYWORDS {
        let in_class = stems.iter().filter(|stem| mentions(&class_lower, stem)).count();
        let in_context = stems.iter().filter(|stem| mentions(&other_lower, stem)).count();
        let score = (in_class, in_context);
        if score != (0, 0) && best.is_none_or(|(_, top)| score > top) {
            best = Some((profile, score));
        }
    }
    best.map(|(profile, _)| profile)
}

/// The block that belongs at the class's NEXT position under `profile`'s chain,
/// given the types already generated (`prior`). Returns `None` when the chain
/// is exhausted (mastery loops legitimately run past it) or nothing in it is
/// currently allowed. Skips what the generator could not use anyway: a repeat
/// of the previous block, a closing block before mastery (`can_close`), and
/// anything in `avoid` (types already dominating the course's earlier classes).
/// `SpacedInterleavedRetrieval` openers do not consume a chain position.
pub fn suggest_next_block(
    profile: DisciplineProfile,
    prior: &[DynamicBlockType],
    can_close: bool,
    avoid: &[DynamicBlockType],
) -> Option<DynamicBlockType> {
    let seq = suggested_sequence(profile);
    let done: Vec<DynamicBlockType> = prior.iter().copied().filter(|t| *t != DynamicBlockType::SpacedInterleavedRetrieval).collect();
    let last = done.last().copied();
    let allowed = |t: DynamicBlockType| {
        Some(t) != last && (can_close || t != DynamicBlockType::MetacognitiveClosure) && !avoid.contains(&t)
    };
    seq.iter().skip(done.len()).copied().find(|t| allowed(*t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::notebook::DynamicBlockType;

    fn index_of(seq: &[DynamicBlockType], t: DynamicBlockType) -> Option<usize> {
        seq.iter().position(|&b| b == t)
    }

    #[test]
    fn every_profile_round_trips_through_as_str_and_parse() {
        for profile in DisciplineProfile::ALL {
            assert_eq!(DisciplineProfile::parse(profile.as_str()), Some(profile));
        }
        assert_eq!(DisciplineProfile::parse("not_a_real_profile"), None);
    }

    #[test]
    fn every_profile_sequence_is_within_3_to_5_blocks() {
        for profile in DisciplineProfile::ALL {
            let seq = suggested_sequence(profile);
            assert!(
                (3..=5).contains(&seq.len()),
                "{profile:?} sequence has {} blocks, expected 3-5",
                seq.len()
            );
        }
    }

    #[test]
    fn no_profile_repeats_a_block_type_consecutively() {
        for profile in DisciplineProfile::ALL {
            let seq = suggested_sequence(profile);
            for pair in seq.windows(2) {
                assert_ne!(pair[0], pair[1], "{profile:?} repeats {:?} consecutively", pair[0]);
            }
        }
    }

    #[test]
    fn spatial_biological_pairs_theory_with_its_own_diagram_then_gates_before_closing() {
        let seq = suggested_sequence(DisciplineProfile::SpatialBiological);
        assert_eq!(seq.first(), Some(&DynamicBlockType::AnchoredMicroTheory));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
        assert!(index_of(seq, DynamicBlockType::AnchoredMicroTheory) < index_of(seq, DynamicBlockType::InteractivePredictionGate));
    }

    #[test]
    fn quantitative_math_builds_a_worked_example_toward_hands_on_practice() {
        let seq = suggested_sequence(DisciplineProfile::QuantitativeMath);
        assert_eq!(seq.first(), Some(&DynamicBlockType::AnchoredMicroTheory));
        assert_eq!(seq.last(), Some(&DynamicBlockType::HandsOnMission));
        assert!(index_of(seq, DynamicBlockType::InteractivePredictionGate) < index_of(seq, DynamicBlockType::HeuristicErrorAudit));
    }

    #[test]
    fn cyclic_systemic_maps_causal_chain_before_auditing_bottlenecks() {
        let seq = suggested_sequence(DisciplineProfile::CyclicSystemic);
        assert!(index_of(seq, DynamicBlockType::DeclarativeVisualDiagram) < index_of(seq, DynamicBlockType::HeuristicErrorAudit));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn software_debugging_audits_the_bug_before_the_hands_on_mission() {
        let seq = suggested_sequence(DisciplineProfile::SoftwareDebugging);
        assert_eq!(seq.first(), Some(&DynamicBlockType::HeuristicErrorAudit));
        assert!(index_of(seq, DynamicBlockType::HeuristicErrorAudit) < index_of(seq, DynamicBlockType::HandsOnMission));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn leadership_decisions_frames_the_branch_before_justifying_the_choice() {
        let seq = suggested_sequence(DisciplineProfile::LeadershipDecisions);
        assert!(index_of(seq, DynamicBlockType::BranchingScenarioChallenge) < index_of(seq, DynamicBlockType::HandsOnMission));
        assert_eq!(seq.last(), Some(&DynamicBlockType::MetacognitiveClosure));
    }

    #[test]
    fn infers_the_profile_from_class_and_course_text() {
        assert_eq!(infer_profile("Depurar un bug en Rust", &["Curso de programación"]), Some(DisciplineProfile::SoftwareDebugging));
        assert_eq!(infer_profile("Derivadas e integrales", &[]), Some(DisciplineProfile::QuantitativeMath));
        assert_eq!(infer_profile("Anatomía de la célula", &["Biología"]), Some(DisciplineProfile::SpatialBiological));
        assert_eq!(infer_profile("Liderazgo en crisis", &["Gestión de equipos"]), Some(DisciplineProfile::LeadershipDecisions));
        assert_eq!(infer_profile("Introducción", &["Bienvenida"]), None);
    }

    #[test]
    fn the_class_title_outweighs_the_course_context() {
        // The course is about code, but today's class is a derivative.
        assert_eq!(
            infer_profile("Derivadas", &["Curso de programación en Rust y Python", "software"]),
            Some(DisciplineProfile::QuantitativeMath)
        );
    }

    #[test]
    fn suggests_the_chain_position_matching_the_blocks_so_far() {
        use DynamicBlockType::*;
        let p = DisciplineProfile::QuantitativeMath;
        assert_eq!(suggest_next_block(p, &[], false, &[]), Some(AnchoredMicroTheory));
        assert_eq!(suggest_next_block(p, &[AnchoredMicroTheory], false, &[]), Some(InteractivePredictionGate));
        // A retrieval opener does not consume a chain position.
        assert_eq!(suggest_next_block(p, &[SpacedInterleavedRetrieval, AnchoredMicroTheory], false, &[]), Some(InteractivePredictionGate));
        // Chain exhausted: no suggestion, mastery decides.
        assert_eq!(suggest_next_block(p, &[AnchoredMicroTheory, InteractivePredictionGate, HeuristicErrorAudit, HandsOnMission], false, &[]), None);
    }

    #[test]
    fn never_suggests_a_closure_before_mastery_nor_a_dominant_type() {
        use DynamicBlockType::*;
        let p = DisciplineProfile::CyclicSystemic;
        let prior = [AnchoredMicroTheory, DeclarativeVisualDiagram, HeuristicErrorAudit];
        assert_eq!(suggest_next_block(p, &prior, false, &[]), None);
        assert_eq!(suggest_next_block(p, &prior, true, &[]), Some(MetacognitiveClosure));
        // HeuristicErrorAudit dominates earlier classes -> skipped for this slot.
        assert_eq!(suggest_next_block(p, &[AnchoredMicroTheory, DeclarativeVisualDiagram], false, &[HeuristicErrorAudit]), None);
    }
}
