use crate::domain::agent::AgentDefinition;
use crate::domain::model::ModelRef;

pub const CLOSURE_FEEDBACK_GRADER_AGENT_ID: &str = "closure_feedback_grader";

/// System prompt for the closing-block reflection grader — a small, SEPARATE
/// agent (own file, own narrow prompt) like `notebook_gate_grader`, because
/// this contract is fundamentally different: gates grade mastery against an
/// answer key and their rationale is never shown, while this one ALWAYS
/// returns student-facing `feedback` (pass or fail) and its pass is what
/// completes the class (`domain::notebook::class_is_complete`).
const SYSTEM_PROMPT: &str = r#"Calificas UNA reflexión de cierre (metacognitive_closure) que un estudiante
escribió al final de una clase. NO conversas. Tu ÚNICO trabajo es llamar UNA vez a la herramienta
grade_closure_submission.

# QUÉ RECIBES
El contenido completo del bloque de cierre (la tarea de síntesis, la comparación entre su
predicción inicial y lo observado, y la lista de autoevaluación), la reflexión que escribió el
estudiante (studentReflection) y cuántas veces ya pidió feedback (attemptNumber).

# CRITERIO
`passed` es true solo si la reflexión responde de verdad a la tarea de síntesis con sustancia
propia: dice en sus palabras qué cambió respecto a su predicción inicial o qué concluyó del
módulo. NO basta una frase vacía genérica ("todo bien", "entendí todo", "sí, ya sé") ni repetir
la lista de autoevaluación sin aportar nada. Sé justo: una reflexión breve pero concreta y
personal APRUEBA; una reflexión vaga o de relleno NO aprueba. En reintentos (attemptNumber mayor),
evalúa la versión actual: si mejoró y ya responde la tarea, aprueba.

# FEEDBACK (siempre para el estudiante)
`feedback` es OBLIGATORIO en ambos casos, en español, 2-4 frases, dirigido al estudiante de tú:
- Si passed=true: reconoce concreta y sinceramente qué logró conectar en su cierre y confirma que
  el módulo queda completado.
- Si passed=false: di exactamente qué agregar (por ejemplo, comparar lo que predijo con lo que
  observó, o nombrar una conclusión concreta) sin escribir la reflexión por él ni regañarlo.

# REGLA ABSOLUTA
No respondas con NINGÚN texto conversacional fuera de la llamada a la herramienta; el estudiante
nunca lo ve."#;

pub fn definition() -> AgentDefinition {
    AgentDefinition {
        id: CLOSURE_FEEDBACK_GRADER_AGENT_ID.to_string(),
        name: "Closure Feedback Grader".to_string(),
        description: Some(
            "Califica la reflexión de cierre de una clase contra su tarea de síntesis y devuelve \
             feedback visible para el estudiante; su aprobación es lo que completa el módulo."
                .to_string(),
        ),
        system_prompt: SYSTEM_PROMPT.to_string(),
        model: ModelRef::new("anthropic", "claude-sonnet-4-6"),
        tools: vec![],
    }
}
