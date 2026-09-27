    use async_trait::async_trait;

    use super::*;
    use crate::agents::AgentRegistry;
    use crate::error::AppResult as Result;
    use crate::orchestration::PromptRunner;
    use crate::providers::factory::PromptOutput;

    struct ScriptedReplyRunner {
        text: String,
    }

    #[async_trait]
    impl PromptRunner for ScriptedReplyRunner {
        async fn run(&self, _: &str, _: &str, _: &str, _input: &str, _: &[String]) -> Result<PromptOutput> {
            Ok(PromptOutput { text: self.text.clone(), tool_calls: vec![] })
        }
    }

    fn service_with(text: &str) -> LexicalAssistantService {
        let agents = Arc::new(AgentRegistry::new());
        agents.register(crate::agents::lexical_assistant_agent::definition()).expect("register lexical assistant agent");
        let orchestrator = Arc::new(Orchestrator::new(agents, Arc::new(ScriptedReplyRunner { text: text.to_string() })));
        LexicalAssistantService::new(orchestrator)
    }

    /// Cheap prompt-regression guard: the anti-shortcut protocol and the
    /// 3-layer format are load-bearing enough that an accidental prompt
    /// rewrite dropping them should fail CI, not just be caught by eye.
    #[test]
    fn system_prompt_contains_the_required_anti_shortcut_and_format_instructions() {
        let def = crate::agents::lexical_assistant_agent::definition();
        let prompt = def.system_prompt.to_lowercase();
        assert!(prompt.contains("prohibido"), "must forbid answering shortcut questions");
        assert!(prompt.contains("analogía cotidiana"), "must mandate the 3-layer format");
        assert!(prompt.contains("mecanismo directo"));
        assert!(prompt.contains("límite de la metáfora"));
    }

    #[tokio::test]
    async fn a_compliant_three_layer_reply_passes_through_unchanged() {
        let reply = "Piensa en una fila del supermercado (analogía). \
                     El mecanismo: cada elemento se procesa en el orden en que llegó. \
                     Límite: la metáfora no explica qué pasa si dos elementos llegan al mismo tiempo.";
        let service = service_with(reply);
        let out = service.ask(None, "cola FIFO", None, &["colas".to_string()], &[], "¿qué es una cola FIFO?", &["B".to_string()]).await.expect("ok");
        assert_eq!(out, reply);
    }

    #[tokio::test]
    async fn a_reply_leaking_an_answer_bearing_string_verbatim_is_redacted() {
        // Simulates a non-compliant model reply that ignores the prompt and
        // names the gate's actual correct option text.
        let leaking_reply = "La respuesta correcta es usar transporte activo secundario porque consume ATP indirectamente.";
        let service = service_with(leaking_reply);
        let answer_bearing = vec!["transporte activo secundario".to_string()];
        let out = service
            .ask(None, "transporte activo", None, &["membranas".to_string()], &[], "¿cuál es la opción correcta?", &answer_bearing)
            .await
            .expect("ok");
        assert_ne!(out, leaking_reply, "the leaked answer text must never reach the client verbatim");
        assert!(out.to_lowercase().contains("no puedo dártela"), "must fall back to the generic refusal");
    }

    #[tokio::test]
    async fn a_reply_matching_a_direct_answer_phrase_is_redacted_even_without_a_known_answer_string() {
        let leaking_reply = "La opción correcta es la B, sin duda.";
        let service = service_with(leaking_reply);
        let out = service.ask(None, "opción", None, &[], &[], "dime cuál elegir", &[]).await.expect("ok");
        assert_ne!(out, leaking_reply);
    }

    #[tokio::test]
    async fn short_answer_bearing_strings_do_not_cause_false_positive_redaction() {
        // A single-letter option like "A" would match almost any reply if
        // not excluded — must not trigger the filter on its own.
        let reply = "Piensa en una carrera (analogía). El mecanismo: cada paso depende del anterior. \
                     Límite: no cubre el caso donde dos pasos ocurren en paralelo.";
        let service = service_with(reply);
        let out = service.ask(None, "secuencia", None, &[], &[], "¿qué es una secuencia?", &["A".to_string()]).await.expect("ok");
        assert_eq!(out, reply, "a trivially short answer string must not redact an unrelated compliant reply");
    }
