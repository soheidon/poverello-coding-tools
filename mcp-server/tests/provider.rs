use std::time::Duration;

use anthro_bridge_mcp_server::provider::deepseek::DeepSeekProvider;
use anthro_bridge_mcp_server::provider::{PlannerProvider, ProviderError};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_provider(endpoint: &str, api_key: &str) -> DeepSeekProvider {
    DeepSeekProvider::new(api_key).with_endpoint(format!("{endpoint}/v1/messages"))
}

#[tokio::test]
async fn successful_plan_extracts_text_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_1",
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "thinking", "thinking": "hidden reasoning" },
                { "type": "text", "text": "Step 1: add the button" },
                { "type": "text", "text": "Step 2: write a test" }
            ]
        })))
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "test-key");
    let result = provider.plan("system", "user").await.unwrap();

    assert_eq!(result.text, "Step 1: add the button\nStep 2: write a test");
}

#[tokio::test]
async fn request_includes_model_effort_auth_and_split_prompts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "ok" }]
        })))
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "test-key");
    let result = provider.plan("system prompt", "user prompt").await.unwrap();

    assert_eq!(result.text, "ok");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);

    let request = &requests[0];
    assert_eq!(
        request
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok()),
        Some("Bearer test-key")
    );

    let body: serde_json::Value = request.body_json().unwrap();
    assert_eq!(body["model"], "deepseek-v4-pro");
    assert_eq!(body["system"], "system prompt");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "user prompt");
    assert_eq!(body["output_config"]["effort"], "high");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["stream"], false);
    assert!(body["max_tokens"].is_number());
}

#[tokio::test]
async fn non_2xx_becomes_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal failure"))
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "test-key");
    let err = provider.plan("system", "user").await.unwrap_err();

    match err {
        ProviderError::Http { status, .. } => assert_eq!(status, 500),
        other => panic!("expected Http error, got {other:?}"),
    }
}

#[tokio::test]
async fn unauthorized_becomes_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid api key"))
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "bad-key");
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(matches!(err, ProviderError::Auth(_)));
}

#[tokio::test]
async fn api_key_never_appears_in_error_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let key = "super-secret-api-key";
    let provider = mock_provider(&server.uri(), key);
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(!err.to_string().contains(key));
    assert!(!err.to_string().contains("Bearer"));
}

#[tokio::test]
async fn api_key_in_response_body_is_redacted() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_string("authentication failed for super-secret-api-key"),
        )
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "super-secret-api-key");
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(!err.to_string().contains("super-secret-api-key"));
    assert!(!err.to_string().contains("Bearer"));
}

#[tokio::test]
async fn timeout_becomes_timeout_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"content": [{"type": "text", "text": "ok"}]}))
                .set_delay(Duration::from_millis(300)),
        )
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "test-key").with_timeout(Duration::from_millis(50));
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(matches!(err, ProviderError::Timeout(_)));
}

#[tokio::test]
async fn malformed_response_becomes_response_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "test-key");
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(matches!(err, ProviderError::Response(_)));
}

#[tokio::test]
async fn response_without_text_becomes_response_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "thinking", "thinking": "hidden" }]
        })))
        .mount(&server)
        .await;

    let provider = mock_provider(&server.uri(), "test-key");
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(matches!(err, ProviderError::Response(_)));
}

#[tokio::test]
async fn empty_api_key_becomes_config_error() {
    let provider = DeepSeekProvider::new("");
    let err = provider.plan("system", "user").await.unwrap_err();

    assert!(matches!(err, ProviderError::Config(_)));
}

#[tokio::test]
async fn anthropic_direct_uses_official_messages_auth_and_response_contract() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [
                { "type": "thinking", "thinking": "not returned" },
                { "type": "text", "text": "Anthropic plan" }
            ],
            "usage": { "input_tokens": 14, "output_tokens": 3 }
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let provider = DynamicBridgeProvider::new().with_target(target);
    let result = provider.plan("system", "task").await.unwrap();
    assert_eq!(result.text, "Anthropic plan");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(
        request.headers.get("x-api-key").unwrap(),
        "anthropic-test-secret"
    );
    assert_eq!(
        request.headers.get("anthropic-version").unwrap(),
        "2023-06-01"
    );
    assert!(request.headers.get("authorization").is_none());
    let body: serde_json::Value = request.body_json().unwrap();
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["system"], "system");
    assert_eq!(body["messages"][0]["content"], "task");
    assert_eq!(body["stream"], false);
    assert_eq!(body["thinking"]["type"], "adaptive");
    assert_eq!(body["output_config"]["effort"], "high");
}

#[tokio::test]
async fn anthropic_rejects_tool_blocks_mixed_with_text() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [
                { "type": "text", "text": "I will execute a tool for you" },
                { "type": "tool_use", "id": "call_1", "name": "bash", "input": {} }
            ]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let err = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::Response(ref msg) if msg.contains("tool block")));
}

#[tokio::test]
async fn anthropic_rejects_refusal_stop_reason_and_refusal_blocks() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    // Case 1: stop_reason = "refusal"
    let server1 = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "stop_reason": "refusal",
            "content": [
                { "type": "text", "text": "I cannot fulfill this request." }
            ]
        })))
        .mount(&server1)
        .await;

    let target1 = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server1.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let err1 = DynamicBridgeProvider::new()
        .with_target(target1)
        .plan("system", "task")
        .await
        .unwrap_err();
    assert!(matches!(err1, ProviderError::Response(ref msg) if msg.contains("refusal stop_reason")));

    // Case 2: refusal content block
    let server2 = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [
                { "type": "refusal", "message": "I refuse this task due to policy." }
            ]
        })))
        .mount(&server2)
        .await;

    let target2 = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server2.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let err2 = DynamicBridgeProvider::new()
        .with_target(target2)
        .plan("system", "task")
        .await
        .unwrap_err();
    assert!(matches!(err2, ProviderError::Response(ref msg) if msg.contains("refuse this task")));
}

#[tokio::test]
async fn anthropic_rejects_unknown_or_invalid_content_blocks() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [
                { "type": "text", "text": "Valid plan line" },
                { "type": "future_unsupported_block_type", "payload": 123 }
            ]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let err = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::Response(ref msg) if msg.contains("unknown or invalid")));
}

#[tokio::test]
async fn anthropic_concatenates_multiple_text_blocks_and_excludes_thinking() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "Internal reasoning here" },
                { "type": "text", "text": "Step 1: Check repo" },
                { "type": "text", "text": "Step 2: Apply changes" }
            ]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();

    assert_eq!(result.text, "Step 1: Check repo\nStep 2: Apply changes");
}

#[tokio::test]
async fn anthropic_excludes_redacted_thinking_and_returns_text() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "stop_reason": "end_turn",
            "content": [
                { "type": "redacted_thinking", "data": "c29tZSByZWRhY3RlZCBieXRlcw==" },
                { "type": "text", "text": "Plan after redacted thinking" }
            ]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();

    assert_eq!(result.text, "Plan after redacted thinking");
}

#[tokio::test]
async fn anthropic_rejects_tool_use_only_response() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "stop_reason": "tool_use",
            "content": [
                { "type": "tool_use", "id": "call_only", "name": "editor", "input": {} }
            ]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let err = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::Response(ref msg) if msg.contains("tool block")));
}

#[tokio::test]
async fn anthropic_rejects_malformed_content_block_with_missing_text() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [
                { "type": "text" }
            ]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "anthropic".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "anthropic-test-secret".into(),
        model: "claude-opus-5-5".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let err = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::Response(ref msg) if msg.contains("failed to parse provider response")));
}

#[tokio::test]
async fn anthropic_rejects_unsupported_thinking_combinations_before_send() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    for (model, mode, effort) in [
        ("claude-opus-5-5", "disabled", Some("high")),
        ("claude-sonnet-5-5", "normal", Some("max")),
        ("claude-haiku-5-5", "disabled", Some("xhigh")),
        ("claude-opus-4-8", "thinking", None),
    ] {
        let target = ResolvedMcpTarget {
            provider_id: "anthropic".into(),
            endpoint: format!("{}/v1/messages", server.uri()),
            api_key: "anthropic-test-secret".into(),
            model: model.into(),
            thinking_mode: Some(mode.into()),
            reasoning_effort: effort.map(str::to_owned),
            is_openrouter: false,
        };
        let err = DynamicBridgeProvider::new()
            .with_target(target)
            .plan("system", "task")
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Config(_)));
    }
    assert_eq!(server.received_requests().await.unwrap().len(), 0);
}

#[tokio::test]
async fn anthropic_is_resolved_from_mcp_provider_config_before_dispatch() {
    use anthro_bridge_mcp_server::provider::adapter::resolve_mcp_target;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let suffix = NEXT.fetch_add(1, Ordering::Relaxed);
    let env_name = format!("MCP_TEST_ANTHROPIC_KEY_{}_{}", std::process::id(), suffix);
    std::env::set_var(&env_name, "anthropic-config-test-secret");

    let config_file = std::env::temp_dir().join(format!(
        "mcp_anthropic_config_{}_{}.json",
        std::process::id(),
        suffix
    ));
    let config = serde_json::json!({
        "active_provider": "deepseek",
        "providers": {
            "anthropic": {
                "upstream_url": "https://untrusted.invalid/custom",
                "api_key_env": env_name,
                "default_model": "claude-opus-5-5",
                "models": {
                    "claude-opus-5": {
                        "upstream_model": "claude-opus-5-5",
                        "thinking_mode": "thinking",
                        "reasoning_effort": "high"
                    }
                }
            }
        },
        "mcp": { "provider": "anthropic" }
    });
    let mut file = std::fs::File::create(&config_file).unwrap();
    file.write_all(config.to_string().as_bytes()).unwrap();

    let resolved = resolve_mcp_target(&config_file).unwrap();
    assert_eq!(resolved.provider_id, "anthropic");
    assert_eq!(resolved.endpoint, "https://api.anthropic.com/v1/messages");
    assert_eq!(resolved.model, "claude-opus-5-5");
    assert_eq!(resolved.api_key, "anthropic-config-test-secret");
    let _ = std::fs::remove_file(config_file);
    std::env::remove_var(env_name);
}

#[tokio::test]
async fn glm_auth_headers_strictly_require_x_api_key_and_reject_bearer() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    // Mock only matches when x-api-key and anthropic-version are present
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-test-secret"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "message",
            "content": [{ "type": "text", "text": "glm plan ok" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-test-secret".into(),
        model: "glm-5.3".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();
    assert_eq!(result.text, "glm plan ok");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req.headers.get("x-api-key").unwrap(), "glm-test-secret");
    assert_eq!(req.headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert!(
        req.headers.get("authorization").is_none(),
        "Must NOT send Authorization: Bearer for zai"
    );
}

#[tokio::test]
async fn glm_53_dispatches_with_thinking_and_reasoning_effort_and_correct_auth_headers() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-53-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "message",
            "content": [{ "type": "text", "text": "glm-5.3 plan" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-53-key".into(),
        model: "glm-5.3".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();
    assert_eq!(result.text, "glm-5.3 plan");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req.headers.get("x-api-key").unwrap(), "glm-53-key");
    assert_eq!(req.headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert!(req.headers.get("authorization").is_none());

    let body: serde_json::Value = req.body_json().unwrap();
    assert_eq!(body["model"], "glm-5.3");
    assert_eq!(body["system"], "system");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "task");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["reasoning_effort"], "high");
    assert_eq!(body["stream"], false);
    assert!(body.get("choices").is_none());
}

#[tokio::test]
async fn glm_53_flash_dispatches_with_thinking_and_reasoning_effort_and_correct_auth_headers() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-flash-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "message",
            "content": [{ "type": "text", "text": "glm-5.3-flash plan" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-flash-key".into(),
        model: "glm-5.3-flash".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("low".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();
    assert_eq!(result.text, "glm-5.3-flash plan");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req.headers.get("x-api-key").unwrap(), "glm-flash-key");
    assert_eq!(req.headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert!(req.headers.get("authorization").is_none());

    let body: serde_json::Value = req.body_json().unwrap();
    assert_eq!(body["model"], "glm-5.3-flash");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(body["stream"], false);
}

#[tokio::test]
async fn glm_53_flashx_dispatches_with_thinking_and_reasoning_effort_and_correct_auth_headers() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-flashx-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "message",
            "content": [{ "type": "text", "text": "glm-5.3-flashx plan" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-flashx-key".into(),
        model: "glm-5.3-flashx".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("max".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();
    assert_eq!(result.text, "glm-5.3-flashx plan");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req.headers.get("x-api-key").unwrap(), "glm-flashx-key");
    assert_eq!(req.headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert!(req.headers.get("authorization").is_none());

    let body: serde_json::Value = req.body_json().unwrap();
    assert_eq!(body["model"], "glm-5.3-flashx");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["reasoning_effort"], "max");
    assert_eq!(body["stream"], false);
}

#[tokio::test]
async fn glm_52_allows_normal_mode_and_supports_thinking_mode_with_correct_auth_headers() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-52-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "type": "message",
            "content": [{ "type": "text", "text": "ok" }]
        })))
        .mount(&server)
        .await;

    // 1. Normal mode (thinking disabled, no reasoning effort)
    let target_normal = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-52-key".into(),
        model: "glm-5.2".into(),
        thinking_mode: Some("normal".into()),
        reasoning_effort: None,
        is_openrouter: false,
    };
    let res_normal = DynamicBridgeProvider::new()
        .with_target(target_normal)
        .plan("sys", "task")
        .await
        .unwrap();
    assert_eq!(res_normal.text, "ok");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req1 = &requests[0];
    assert_eq!(req1.headers.get("x-api-key").unwrap(), "glm-52-key");
    assert_eq!(req1.headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert!(req1.headers.get("authorization").is_none());
    let body1: serde_json::Value = req1.body_json().unwrap();
    assert_eq!(body1["model"], "glm-5.2");
    assert_eq!(body1["thinking"]["type"], "disabled");
    assert!(body1.get("reasoning_effort").is_none());

    // 2. Thinking mode (thinking enabled, reasoning effort present)
    let target_thinking = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-52-key".into(),
        model: "glm-5.2".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("high".into()),
        is_openrouter: false,
    };
    let res_thinking = DynamicBridgeProvider::new()
        .with_target(target_thinking)
        .plan("sys", "task")
        .await
        .unwrap();
    assert_eq!(res_thinking.text, "ok");

    let requests2 = server.received_requests().await.unwrap();
    assert_eq!(requests2.len(), 2);
    let req2 = &requests2[1];
    assert_eq!(req2.headers.get("x-api-key").unwrap(), "glm-52-key");
    assert_eq!(req2.headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert!(req2.headers.get("authorization").is_none());
    let body2: serde_json::Value = req2.body_json().unwrap();
    assert_eq!(body2["model"], "glm-5.2");
    assert_eq!(body2["thinking"]["type"], "enabled");
    assert_eq!(body2["reasoning_effort"], "high");
}

#[tokio::test]
async fn glm_53_series_rejects_disabled_thinking_before_send() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    // Verify that glm-5.3, glm-5.3-flash, and glm-5.3-flashx all reject disabling thinking
    for model in ["glm-5.3", "glm-5.3-flash", "glm-5.3-flashx"] {
        let target = ResolvedMcpTarget {
            provider_id: "zai".into(),
            endpoint: format!("{}/v1/messages", server.uri()),
            api_key: "glm-test-secret".into(),
            model: model.into(),
            thinking_mode: Some("normal".into()),
            reasoning_effort: None,
            is_openrouter: false,
        };
        let err = DynamicBridgeProvider::new()
            .with_target(target)
            .plan("system", "task")
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProviderError::Config(ref msg) if msg.contains("does not support disabling thinking")),
            "Model {model} must reject disabled thinking"
        );
    }

    // Verify invalid reasoning effort rejection
    let target_invalid_effort = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "glm-test-secret".into(),
        model: "glm-5.3".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("xhigh".into()),
        is_openrouter: false,
    };
    let err_effort = DynamicBridgeProvider::new()
        .with_target(target_invalid_effort)
        .plan("system", "task")
        .await
        .unwrap_err();
    assert!(matches!(err_effort, ProviderError::Config(ref msg) if msg.contains("GLM reasoning effort")));

    // Verify unsupported model rejection
    let target_bad = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "glm-test-secret".into(),
        model: "glm-unknown".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("max".into()),
        is_openrouter: false,
    };
    let err_model = DynamicBridgeProvider::new()
        .with_target(target_bad)
        .plan("sys", "task")
        .await
        .unwrap_err();
    assert!(matches!(err_model, ProviderError::Config(ref msg) if msg.contains("Unsupported model")));

    // No request reached the server
    assert_eq!(server.received_requests().await.unwrap().len(), 0);
}

#[tokio::test]
async fn glm_handles_thinking_only_max_tokens_response() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-test-secret"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_glm_flash",
            "type": "message",
            "role": "assistant",
            "model": "glm-5.3-flash",
            "stop_reason": "max_tokens",
            "content": [{ "type": "thinking", "thinking": "deep thoughts" }],
            "usage": { "input_tokens": 16, "output_tokens": 64 }
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-test-secret".into(),
        model: "glm-5.3-flash".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("max".into()),
        is_openrouter: false,
    };
    let err = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap_err();
    match err {
        anthro_bridge_mcp_server::provider::ProviderError::Response(msg) => {
            assert!(
                msg.contains("no text content") || msg.contains("max_tokens"),
                "expected response error mentioning no text or max_tokens, got: {msg}"
            );
        }
        other => panic!("expected ProviderError::Response, got {other:?}"),
    }

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req.headers.get("x-api-key").unwrap(), "glm-test-secret");
    assert_eq!(req.headers.get("anthropic-version").unwrap(), "2023-06-01");
}

#[tokio::test]
async fn glm_handles_thinking_plus_final_text_response() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-test-secret"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_glm_flash_ok",
            "type": "message",
            "role": "assistant",
            "model": "glm-5.3-flash",
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "deep thoughts" },
                { "type": "text", "text": "final validated plan" }
            ],
            "usage": { "input_tokens": 16, "output_tokens": 64 }
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-test-secret".into(),
        model: "glm-5.3-flash".into(),
        thinking_mode: Some("thinking".into()),
        reasoning_effort: Some("max".into()),
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();
    assert_eq!(result.text, "final validated plan");
    assert!(!result.text.contains("deep thoughts"));
}

#[tokio::test]
async fn glm_handles_regular_text_response() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/anthropic/v1/messages"))
        .and(header("x-api-key", "glm-test-secret"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_glm_52",
            "type": "message",
            "role": "assistant",
            "model": "glm-5.2",
            "stop_reason": "end_turn",
            "content": [
                { "type": "text", "text": "ordinary text output" }
            ],
            "usage": { "input_tokens": 10, "output_tokens": 20 }
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "zai".into(),
        endpoint: format!("{}/api/anthropic/v1/messages", server.uri()),
        api_key: "glm-test-secret".into(),
        model: "glm-5.2".into(),
        thinking_mode: Some("normal".into()),
        reasoning_effort: None,
        is_openrouter: false,
    };
    let result = DynamicBridgeProvider::new()
        .with_target(target)
        .plan("system", "task")
        .await
        .unwrap();
    assert_eq!(result.text, "ordinary text output");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert!(req.headers.get("authorization").is_none());
}


#[tokio::test]
async fn glm_resolver_uses_the_anthropic_messages_endpoint_only() {
    use anthro_bridge_mcp_server::provider::adapter::resolve_mcp_target;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let env_name = format!("MCP_TEST_GLM_KEY_{}_{}", std::process::id(), id);
    std::env::set_var(&env_name, "glm-resolver-test-key");
    let config_file = std::env::temp_dir().join(format!(
        "mcp_glm_config_{}_{}.json",
        std::process::id(),
        id
    ));
    let config = serde_json::json!({
        "active_provider": "deepseek",
        "providers": {
            "zai": {
                "upstream_url": "https://api.z.ai/api/anthropic",
                "api_key_env": env_name,
                "default_model": "glm-5.3",
                "models": { "claude-opus-5": { "upstream_model": "glm-5.3", "thinking_mode": "thinking", "reasoning_effort": "max" } }
            }
        },
        "mcp": { "provider": "zai" }
    });
    std::fs::write(&config_file, config.to_string()).unwrap();
    let resolved = resolve_mcp_target(&config_file).unwrap();
    assert_eq!(resolved.provider_id, "zai");
    assert_eq!(resolved.endpoint, "https://api.z.ai/api/anthropic/v1/messages");
    assert_eq!(resolved.model, "glm-5.3");

    // Without an explicit mcp.provider=zai selection, GLM is never resolved.
    let unselected = serde_json::json!({
        "active_provider": "zai",
        "providers": config["providers"].clone()
    });
    std::fs::write(&config_file, unselected.to_string()).unwrap();
    assert!(matches!(
        resolve_mcp_target(&config_file),
        Err(ProviderError::Config(_))
    ));
    let _ = std::fs::remove_file(config_file);
    std::env::remove_var(env_name);
}

#[tokio::test]
async fn dynamic_bridge_provider_dispatches_configured_target() {
    use anthro_bridge_mcp_server::provider::adapter::{resolve_mcp_target, DynamicBridgeProvider};
    use std::io::Write;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from OpenRouter Gemini" }]
        })))
        .mount(&server)
        .await;

    // Create a temporary config file pointing to our mock server
    let temp_dir = std::env::temp_dir();
    let config_file = temp_dir.join(format!("test_config_{}.json", std::process::id()));

    std::env::set_var("TEST_OPENROUTER_KEY", "sk-openrouter-dynamic-key");

    let cfg_json = serde_json::json!({
        "config_version": "1.0",
        "active_provider": "openrouter",
        "providers": {
            "openrouter": {
                "display_name": "OpenRouter",
                "upstream_url": format!("{}/v1/messages", server.uri()),
                "api_key_env": "TEST_OPENROUTER_KEY",
                "default_model": "google/gemini-2.5-flash",
                "profiles": [
                    {
                        "id": "gemini_prof",
                        "display_name": "Gemini Profile",
                        "models": {
                            "claude-opus-5": {
                                "upstream_model": "google/gemini-2.5-pro",
                                "thinking_mode": "thinking",
                                "reasoning_effort": "low"
                            }
                        }
                    }
                ]
            }
        },
        "mcp": {
            "provider": "openrouter",
            "profile_id": "gemini_prof",
            "model": "google/gemini-2.5-pro",
            "thinking_mode": "thinking",
            "reasoning_effort": "low"
        }
    });

    let mut file = std::fs::File::create(&config_file).unwrap();
    file.write_all(cfg_json.to_string().as_bytes()).unwrap();

    let target = resolve_mcp_target(&config_file).unwrap();
    assert_eq!(target.provider_id, "openrouter");
    assert_eq!(target.model, "google/gemini-2.5-pro");
    assert_eq!(target.thinking_mode.as_deref(), Some("thinking"));
    assert_eq!(target.reasoning_effort.as_deref(), Some("low"));
    assert_eq!(target.api_key, "sk-openrouter-dynamic-key");

    let provider = DynamicBridgeProvider::new().with_config_path(config_file.clone());
    let response = provider.plan("sys", "user").await.unwrap();
    assert_eq!(response.text, "Plan from OpenRouter Gemini");

    let _ = std::fs::remove_file(&config_file);
}

#[tokio::test]
async fn openrouter_gemini_uses_reasoning_and_never_deepseek_output_config() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from OpenRouter Gemini" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "google/gemini-2.5-flash".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("medium".to_string()),
        is_openrouter: true,
    };

    let provider = DynamicBridgeProvider::new().with_target(target);
    let response = provider.plan("sys", "user").await.unwrap();
    assert_eq!(response.text, "Plan from OpenRouter Gemini");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "google/gemini-2.5-flash");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert!(
        body.get("output_config").is_none(),
        "DeepSeek output_config must NOT be attached to OpenRouter requests"
    );
}

#[tokio::test]
async fn deepseek_uses_output_config_effort() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from DeepSeek" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "deepseek".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "deepseek-test-key".to_string(),
        model: "deepseek-v4-pro".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: false,
    };

    let provider = DynamicBridgeProvider::new().with_target(target);
    let response = provider.plan("sys", "user").await.unwrap();
    assert_eq!(response.text, "Plan from DeepSeek");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "deepseek-v4-pro");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["output_config"]["effort"], "high");
}

#[tokio::test]
async fn kimi_k3_suppresses_thinking_and_sends_reasoning_effort() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from Kimi K3" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "kimi".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "kimi-test-key".to_string(),
        model: "kimi-k3".to_string(),
        thinking_mode: Some("thinking_only".to_string()),
        reasoning_effort: Some("max".to_string()),
        is_openrouter: false,
    };

    let provider = DynamicBridgeProvider::new().with_target(target);
    let response = provider.plan("sys", "user").await.unwrap();
    assert_eq!(response.text, "Plan from Kimi K3");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "kimi-k3");
    assert!(body.get("thinking").is_none());
    assert_eq!(body["reasoning_effort"], "max");
}

#[tokio::test]
async fn kimi_code_thinking_payload() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    for model_name in ["kimi-for-coding", "kimi-for-coding-highspeed"] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{ "type": "text", "text": "Plan from Kimi Code" }]
            })))
            .mount(&server)
            .await;

        let target = ResolvedMcpTarget {
            provider_id: "kimi-code".to_string(),
            endpoint: format!("{}/v1/messages", server.uri()),
            api_key: "kimi-code-test-key".to_string(),
            model: model_name.to_string(),
            thinking_mode: Some("thinking_only".to_string()),
            reasoning_effort: None,
            is_openrouter: false,
        };

        let provider = DynamicBridgeProvider::new().with_target(target);
        let response = provider.plan("sys", "user").await.unwrap();
        assert_eq!(response.text, "Plan from Kimi Code");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], model_name);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert!(
            body.get("reasoning_effort").is_none(),
            "reasoning_effort must not be present for {}",
            model_name
        );
        assert!(
            body.get("reasoning").is_none(),
            "reasoning must not be present for {}",
            model_name
        );
        assert!(
            body.get("output_config").is_none(),
            "output_config must not be present for {}",
            model_name
        );
    }
}

#[tokio::test]
async fn minimax_omits_output_config() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from MiniMax" }]
        })))
        .mount(&server)
        .await;

    let target = ResolvedMcpTarget {
        provider_id: "minimax".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "minimax-test-key".to_string(),
        model: "MiniMax-M3".to_string(),
        thinking_mode: Some("thinking_only".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: false,
    };

    let provider = DynamicBridgeProvider::new().with_target(target);
    let response = provider.plan("sys", "user").await.unwrap();
    assert_eq!(response.text, "Plan from MiniMax");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "MiniMax-M3");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert!(body.get("output_config").is_none());
}

#[tokio::test]
async fn openrouter_deepseek_v4_1_flash_reasoning_payloads() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from DeepSeek V4.1 Flash" }]
        })))
        .mount(&server)
        .await;

    // 1. Thinking with low effort -> effort: "low"
    let target_low = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4.1-flash".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("low".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_low);
    let res = provider.plan("sys", "user").await.unwrap();
    assert_eq!(res.text, "Plan from DeepSeek V4.1 Flash");

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "deepseek/deepseek-v4.1-flash");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert!(body.get("output_config").is_none());

    // 2. Thinking with xhigh effort -> effort: "max"
    let target_xhigh = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4.1-flash".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("xhigh".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_xhigh);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value = requests[1].body_json().unwrap();
    assert_eq!(body["reasoning"]["effort"], "max");

    // 3. Normal mode -> reasoning enabled: false
    let target_normal = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4.1-flash".to_string(),
        thinking_mode: Some("normal".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_normal);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    let body: serde_json::Value = requests[2].body_json().unwrap();
    assert_eq!(body["reasoning"]["enabled"], false);
    assert!(body["reasoning"].get("effort").is_none());

    // 4. Unset mode -> no reasoning injected
    let target_unset = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4.1-flash".to_string(),
        thinking_mode: None,
        reasoning_effort: None,
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_unset);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    let body: serde_json::Value = requests[3].body_json().unwrap();
    assert!(body.get("reasoning").is_none());
    assert!(body.get("output_config").is_none());
}

#[tokio::test]
async fn openrouter_deepseek_v4_flash_0731_reasoning_payloads() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from DeepSeek V4 Flash 0731" }]
        })))
        .mount(&server)
        .await;

    // 1. Thinking with low effort -> normalized to "high"
    let target_low = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-flash-0731".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("low".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_low);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "deepseek/deepseek-v4-flash-0731");
    assert_eq!(body["reasoning"]["effort"], "high");

    // 2. Thinking with high effort -> "high"
    let target_high = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-flash-0731".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_high);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value = requests[1].body_json().unwrap();
    assert_eq!(body["reasoning"]["effort"], "high");

    // 3. Thinking with xhigh effort -> "max"
    let target_xhigh = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-flash-0731".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("xhigh".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_xhigh);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    let body: serde_json::Value = requests[2].body_json().unwrap();
    assert_eq!(body["reasoning"]["effort"], "max");

    // 4. Normal mode -> reasoning enabled: false
    let target_normal = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-flash-0731".to_string(),
        thinking_mode: Some("normal".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_normal);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    let body: serde_json::Value = requests[3].body_json().unwrap();
    assert_eq!(body["reasoning"]["enabled"], false);
    assert!(body["reasoning"].get("effort").is_none());
}

#[tokio::test]
async fn openrouter_deepseek_v4_pro_0813_reasoning_payloads() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from DeepSeek V4 Pro 0813" }]
        })))
        .mount(&server)
        .await;

    // 1. Thinking with low effort -> normalized to "high"
    let target_low = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-pro-0813".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("low".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_low);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "deepseek/deepseek-v4-pro-0813");
    assert_eq!(body["reasoning"]["effort"], "high");

    // 2. Thinking with high effort -> "high"
    let target_high = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-pro-0813".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_high);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value = requests[1].body_json().unwrap();
    assert_eq!(body["reasoning"]["effort"], "high");

    // 3. Thinking with xhigh effort -> "max"
    let target_xhigh = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-pro-0813".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("xhigh".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_xhigh);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    let body: serde_json::Value = requests[2].body_json().unwrap();
    assert_eq!(body["reasoning"]["effort"], "max");

    // 4. Normal mode -> reasoning enabled: false
    let target_normal = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-pro-0813".to_string(),
        thinking_mode: Some("normal".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_normal);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    let body: serde_json::Value = requests[3].body_json().unwrap();
    assert_eq!(body["reasoning"]["enabled"], false);
    assert!(body["reasoning"].get("effort").is_none());
}

#[tokio::test]
async fn openrouter_deepseek_unknown_slug_does_not_enter_deepseek_branch() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from Unknown Model" }]
        })))
        .mount(&server)
        .await;

    let target_unknown = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "deepseek/deepseek-v4-unknown".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("low".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_unknown);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "deepseek/deepseek-v4-unknown");
    // Standard OpenRouter fallback sends thinking: { type: "enabled" } and reasoning: { effort: "low" }
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["reasoning"]["effort"], "low");
}

#[tokio::test]
async fn openrouter_openai_astra_payloads() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "Plan from OpenAI Astra" }]
        })))
        .mount(&server)
        .await;

    // 1. GPT-6 Astra with high effort
    let target_astra = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "openai/gpt-6-astra".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("high".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_astra);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "openai/gpt-6-astra");
    assert_eq!(body["reasoning"]["effort"], "high");
    assert!(
        body.get("thinking").is_none(),
        "Must NOT send Anthropic thinking envelope"
    );

    // 2. GPT-6 Astra Pro (dedicated Pro endpoint, no effort or thinking envelope)
    let target_pro = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "openai/gpt-6-astra-pro".to_string(),
        thinking_mode: Some("thinking_only".to_string()),
        reasoning_effort: None,
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_pro);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value = requests[1].body_json().unwrap();
    assert_eq!(body["model"], "openai/gpt-6-astra-pro");
    assert!(
        body.get("reasoning").is_none(),
        "Astra Pro must NOT have reasoning.effort injected"
    );
    assert!(
        body.get("thinking").is_none(),
        "Astra Pro must NOT have thinking envelope"
    );

    // 3. GPT Astra Latest with max effort
    let target_latest = ResolvedMcpTarget {
        provider_id: "openrouter".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "openrouter-test-key".to_string(),
        model: "openai/gpt-astra-latest".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: Some("max".to_string()),
        is_openrouter: true,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_latest);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    let body: serde_json::Value = requests[2].body_json().unwrap();
    assert_eq!(body["model"], "openai/gpt-astra-latest");
    assert_eq!(body["reasoning"]["effort"], "max");
    assert!(body.get("thinking").is_none());
}

#[tokio::test]
async fn dynamic_provider_handles_mimo_v2_6_thinking_and_normal() {
    use anthro_bridge_mcp_server::provider::adapter::{DynamicBridgeProvider, ResolvedMcpTarget};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": [{ "type": "text", "text": "plan" }]
        })))
        .mount(&server)
        .await;

    // 1. MiMo V2.6 Pro with Thinking
    let target_thinking = ResolvedMcpTarget {
        provider_id: "mimo".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "mimo-test-key".to_string(),
        model: "mimo-v2.6-pro".to_string(),
        thinking_mode: Some("thinking".to_string()),
        reasoning_effort: None,
        is_openrouter: false,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_thinking);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(body["model"], "mimo-v2.6-pro");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert!(body.get("output_config").is_none());
    assert!(body.get("reasoning_effort").is_none());

    // 2. MiMo V2.6 Flash with Normal (thinking disabled)
    let target_normal = ResolvedMcpTarget {
        provider_id: "mimo".to_string(),
        endpoint: format!("{}/v1/messages", server.uri()),
        api_key: "mimo-test-key".to_string(),
        model: "mimo-v2.6-flash".to_string(),
        thinking_mode: Some("normal".to_string()),
        reasoning_effort: None,
        is_openrouter: false,
    };
    let provider = DynamicBridgeProvider::new().with_target(target_normal);
    provider.plan("sys", "user").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let body: serde_json::Value = requests[1].body_json().unwrap();
    assert_eq!(body["model"], "mimo-v2.6-flash");
    assert_eq!(body["thinking"]["type"], "disabled");
    assert!(body.get("output_config").is_none());
    assert!(body.get("reasoning_effort").is_none());
}
