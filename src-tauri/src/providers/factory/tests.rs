    use super::*;

    fn openai_cfg() -> ProviderConfig {
        ProviderConfig {
            id: "openai".to_string(),
            provider: ProviderKind::OpenAI,
            name: "OpenAI".to_string(),
            default_model: None,
            base_url: None,
        }
    }

    #[test]
    fn prompt_timeout_parses_clamps_and_falls_back() {
        assert_eq!(parse_prompt_timeout(None), 200);
        assert_eq!(parse_prompt_timeout(Some("abc")), 200);
        assert_eq!(parse_prompt_timeout(Some("")), 200);
        assert_eq!(parse_prompt_timeout(Some(" 300 ")), 300);
        assert_eq!(parse_prompt_timeout(Some("5")), 30);
        assert_eq!(parse_prompt_timeout(Some("99999")), 900);
    }

    #[test]
    fn resolve_model_prefers_override_then_default() {
        let mut cfg = openai_cfg();
        assert!(matches!(
            ProviderFactory::resolve_model(&cfg, None),
            Err(AppError::ModelNotConfigured(_))
        ));
        cfg.default_model = Some("gpt-5".to_string());
        assert_eq!(ProviderFactory::resolve_model(&cfg, None).expect("ok"), "gpt-5");
        assert_eq!(
            ProviderFactory::resolve_model(&cfg, Some("other")).expect("ok"),
            "other"
        );
    }

    #[tokio::test]
    async fn rejects_empty_key_without_network() {
        let cfg = openai_cfg();
        let err = ProviderFactory::verify_connection(&cfg, "   ").await.expect_err("must fail");
        assert!(matches!(err, AppError::ProviderKeyMissing(_)));
        let err = ProviderFactory::list_models(&cfg, "").await.expect_err("must fail");
        assert!(matches!(err, AppError::ProviderKeyMissing(_)));
    }

    #[tokio::test]
    async fn rejects_unknown_tool_without_network() {
        let mut cfg = openai_cfg();
        cfg.default_model = Some("gpt-5".to_string());
        let err = ProviderFactory::run_prompt(&cfg, "sk-test", "sys", "hi", &["nope".to_string()])
            .await
            .expect_err("must fail");
        assert!(matches!(err, AppError::ToolNotFound(_)));
    }

    #[test]
    fn error_trimming_redacts_bearer() {
        let msg = trim_error("401 Unauthorized Bearer sk-abc123 tail");
        assert!(!msg.contains("sk-abc123"));
        assert!(msg.contains("[redacted]"));
    }

    #[test]
    fn output_budget_is_set_per_provider() {
        assert_eq!(ProviderFactory::output_budget(&ProviderKind::DeepSeek), 131_072);
        assert_eq!(ProviderFactory::output_budget(&ProviderKind::Anthropic), 16384);
        assert_eq!(ProviderFactory::output_budget(&ProviderKind::OpenAI), 16384);
        assert_eq!(ProviderFactory::output_budget(&ProviderKind::Gemini), 16384);
        assert_eq!(ProviderFactory::output_budget(&ProviderKind::OpenRouter), 16384);
        assert_eq!(ProviderFactory::output_budget(&ProviderKind::Xiaomi), 131_072);
    }

    #[test]
    fn length_truncation_maps_to_spanish_hint() {
        let err = agent_error(&"CompletionError: ResponseError: stopped with finish_reason=Length, raise max_tokens");
        assert!(matches!(err, AppError::AgentExecutionFailed(_)));
        let shown = serde_json::to_value(&err).expect("serializable");
        assert_eq!(shown["code"], "AgentExecutionFailed");
        let message = shown["message"].as_str().unwrap();
        assert!(message.contains("espacio de respuesta"), "hint in Spanish, got: {message}");
        assert!(!message.contains("finish_reason"), "no raw provider English leaks");
    }

    #[test]
    fn agent_errors_reach_the_ui_detail_redacted() {
        let err = agent_error(&"boom Bearer sk-secret tail");
        let shown = serde_json::to_value(&err).expect("serializable");
        let message = shown["message"].as_str().unwrap();
        assert!(message.contains("agent execution failed"), "keeps the typed prefix");
        assert!(!message.contains("sk-secret"));
    }
