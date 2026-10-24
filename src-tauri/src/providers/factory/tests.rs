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
    fn anthropic_models_cache_the_static_prefix_for_an_hour_with_a_moving_tail() {
        use rig_core::providers::anthropic::completion::CacheTtl;
        let client = rig_core::providers::anthropic::Client::builder().api_key("sk-test".to_string()).build().expect("client");
        let plain = client.completion_model("claude-sonnet-4-6");
        assert!(!plain.automatic_caching && plain.static_prefix_cache_ttl.is_none());
        let cached = anthropic_cached(plain);
        assert!(cached.automatic_caching, "top-level breakpoint moves with the conversation tail");
        assert_eq!(cached.static_prefix_cache_ttl, Some(CacheTtl::OneHour));
        assert!(!cached.prompt_caching, "manual last-message marker would duplicate the automatic one");
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
    fn only_the_scopes_that_write_the_whole_plan_get_the_longer_timeout() {
        assert_eq!(proposal_timeout_secs(200), 600, "3x the default");
        assert_eq!(proposal_timeout_secs(900), MAX_PROPOSAL_TIMEOUT_SECS, "capped, never unbounded");
        assert_eq!(proposal_timeout_secs(u64::MAX), MAX_PROPOSAL_TIMEOUT_SECS, "no overflow");
        let base = prompt_timeout_secs();
        assert_eq!(diagnostic_timeout_secs(DiagnosticToolScope::Assessment), base);
        assert_eq!(diagnostic_timeout_secs(DiagnosticToolScope::Battery), base);
        assert_eq!(diagnostic_timeout_secs(DiagnosticToolScope::Propose), proposal_timeout_secs(base));
        assert_eq!(diagnostic_timeout_secs(DiagnosticToolScope::All), proposal_timeout_secs(base));
    }

    #[test]
    fn block_streaming_is_on_unless_explicitly_switched_off() {
        assert!(parse_stream_blocks(None));
        assert!(parse_stream_blocks(Some("1")));
        assert!(parse_stream_blocks(Some("whatever")));
        for off in ["0", "false", "OFF", " no "] {
            assert!(!parse_stream_blocks(Some(off)), "{off:?} must switch streaming off");
        }
    }

    #[tokio::test]
    async fn the_progress_sink_is_visible_only_inside_its_scope() {
        use crate::tools::block_stream::{BLOCK_STREAM_SINK, BlockStreamUpdate};
        let sink: BlockStreamSink = std::sync::Arc::new(|_: BlockStreamUpdate| {});
        assert!(current_sink().is_none(), "no sink outside a scope: callers without a UI keep the blocking path");
        BLOCK_STREAM_SINK.scope(sink, async { assert!(current_sink().is_some()) }).await;
        assert!(current_sink().is_none(), "the scope ends with the attempt");
    }

    /// Drives the REAL rig streaming loop over a scripted model: the capture is
    /// filled by the tool exactly as on the blocking path, the loop stops after
    /// ONE model call, and the sink only ever sees sanitized progress.
    mod streamed_block_turn {
        use super::*;
        use crate::tools::block_stream::BlockStreamUpdate;
        use crate::tools::{NotebookBlockCapture, PublishNotebookBlockTool, stop_when_some};
        use rig_agent::agent::AgentBuilder as Builder;
        use rig_core::test_utils::{MockCompletionModel, MockStreamEvent};
        use std::sync::{Arc, Mutex};

        fn theory() -> serde_json::Value {
            serde_json::json!({
                "blockType": "anchored_micro_theory", "title": "Recursión", "intuitiveHook": "h", "systemRule": "r", "frequentError": "f"
            })
        }

        fn collecting_sink() -> (BlockStreamSink, Arc<Mutex<Vec<BlockStreamUpdate>>>) {
            let seen: Arc<Mutex<Vec<BlockStreamUpdate>>> = Arc::new(Mutex::new(Vec::new()));
            let seen2 = seen.clone();
            (Arc::new(move |u| seen2.lock().unwrap().push(u)), seen)
        }

        async fn run(model: MockCompletionModel, capture: NotebookBlockCapture, sink: BlockStreamSink) -> AppResult<PromptOutput> {
            let agent = Builder::new(model).tool(PublishNotebookBlockTool(capture.clone())).build();
            ProviderFactory::stream_notebook_block(&agent, "go", stop_when_some(capture), sink, BlockCallParams::resolve(None, Some("6"), None)).await
        }

        #[tokio::test]
        async fn a_streamed_tool_call_fills_the_capture_stops_after_one_call_and_streams_only_safe_progress() {
            let model = MockCompletionModel::from_stream_turns([[
                MockStreamEvent::reasoning_delta("pensando en la explicación"),
                MockStreamEvent::tool_call_name_delta("t1", "publish_notebook_block"),
                MockStreamEvent::tool_call_arguments_delta("t1", r#"{"blockType":"anchored_micro_theory","title":"Recursión""#),
                // A leak attempt in a later fragment: the sink must never carry it.
                MockStreamEvent::tool_call_arguments_delta("t1", r#","correctOption":"B"}"#),
                MockStreamEvent::tool_call("t1", "publish_notebook_block", theory()),
                MockStreamEvent::final_response_with_default_usage(),
            ]]);
            let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
            let (sink, seen) = collecting_sink();

            let out = run(model.clone(), capture.clone(), sink).await.expect("our own terminal stop is success");
            assert_eq!(out.tool_calls, vec!["publish_notebook_block".to_string()]);
            assert_eq!(model.request_count(), 1, "the loop ends after the first tool result");
            let published = capture.lock().unwrap().take().expect("the tool filled the capture, as on the blocking path");
            assert_eq!(serde_json::to_value(&published.block).unwrap()["title"], "Recursión");

            let seen = seen.lock().unwrap();
            assert!(seen.iter().any(|u| matches!(u, BlockStreamUpdate::Reasoning { chars } if *chars > 0)), "reasoning progress is reported");
            let previews: Vec<&serde_json::Value> = seen.iter().filter_map(|u| if let BlockStreamUpdate::Preview(p) = u { Some(p) } else { None }).collect();
            assert!(!previews.is_empty(), "the block's text is streamed");
            assert_eq!(previews[0]["blockType"], "anchored_micro_theory");
            assert_eq!(previews[0]["fields"]["title"], "Recursión");
            let everything = format!("{seen:?}");
            assert!(!everything.contains("correctOption"), "an answer key must never reach the sink: {everything}");
            assert!(!everything.contains("pensando"), "reasoning text must never reach the sink");
        }

        #[tokio::test]
        async fn a_stream_error_surfaces_as_an_agent_failure_not_a_success() {
            let model = MockCompletionModel::from_stream_turns([[MockStreamEvent::error("conexión cortada")]]);
            let capture: NotebookBlockCapture = Arc::new(Mutex::new(None));
            let (sink, _) = collecting_sink();
            let err = run(model, capture.clone(), sink).await.expect_err("must fail");
            assert!(matches!(err, AppError::AgentExecutionFailed(_)), "got {err:?}");
            assert!(capture.lock().unwrap().is_none(), "nothing was published");
        }
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

    #[test]
    fn block_call_params_default_and_clamp() {
        let d = BlockCallParams::resolve(None, None, None);
        assert_eq!((d.max_turns, d.timeout_secs, d.require_tool), (3, 420, true));
        let o = BlockCallParams::resolve(Some("600"), Some("50"), Some(" auto "));
        assert_eq!((o.max_turns, o.timeout_secs, o.require_tool), (10, 600, false));
        let bad = BlockCallParams::resolve(Some("x"), Some("0"), Some("required"));
        assert_eq!((bad.max_turns, bad.timeout_secs, bad.require_tool), (1, 420, true));
    }
