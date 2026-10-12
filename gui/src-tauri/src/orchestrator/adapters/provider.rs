use serde_json::{json, Value};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use super::super::process_runner::HTTP_INFERENCE_SAFETY_TIMEOUT;
use super::super::secrets::SecretRedactor;
use super::{AdapterExecutionInput, AdapterExecutionOutput};

static ALLOWED_PROVIDERS: &[&str] = &[
    "mimo",
    "deepseek",
    "kimi",
    "kimi_code",
    "minimax",
    "openrouter",
    "zai",
];

#[derive(Debug, Clone, Default)]
pub struct ProviderAdapter {
    client: reqwest::Client,
    redactor: SecretRedactor,
    #[cfg(test)]
    endpoint_override: Option<String>,
    #[cfg(test)]
    api_key_override: Option<String>,
}

impl ProviderAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder().build().unwrap_or_default(),
            redactor: SecretRedactor::new(),
            #[cfg(test)]
            endpoint_override: None,
            #[cfg(test)]
            api_key_override: None,
        }
    }

    pub async fn execute(
        &self,
        input: &AdapterExecutionInput,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<AdapterExecutionOutput, String> {
        let profile = &input.profile;
        let provider_id = profile.provider_id.as_deref().ok_or_else(|| {
            "Provider profile is missing required provider_id; refusing to select a default provider.".to_string()
        })?;

        // Fail closed for unknown provider IDs before making any HTTP request
        if !ALLOWED_PROVIDERS.contains(&provider_id) {
            return Err(format!(
                "Unsupported provider ID '{}'. Allowed providers: {}",
                provider_id,
                ALLOWED_PROVIDERS.join(", ")
            ));
        }

        let model = if provider_id == "zai" {
            profile.model.as_deref().unwrap_or(crate::ZAI_DEFAULT_MODEL)
        } else {
            profile.model.as_deref().unwrap_or("deepseek-v4.1-flash")
        };
        let (endpoint, api_key_env) = resolve_provider_endpoint_and_env(provider_id)?;
        #[cfg(test)]
        let endpoint = self
            .endpoint_override
            .clone()
            .unwrap_or(endpoint);

        #[cfg(test)]
        let configured_api_key = self.api_key_override.clone();
        #[cfg(not(test))]
        let configured_api_key: Option<String> = None;
        let api_key = configured_api_key
            .or_else(|| crate::resolve_api_key_env_var(&api_key_env))
            .ok_or_else(|| {
                format!(
                    "API key environment variable '{}' is not set for provider '{}'",
                    api_key_env, provider_id
                )
            })?;

        if api_key.trim().is_empty() {
            return Err(format!(
                "API key '{}' is empty for provider '{}'",
                api_key_env, provider_id
            ));
        }

        let mut body = json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": input.system_prompt
                },
                {
                    "role": "user",
                    "content": input.user_prompt
                }
            ],
            "stream": false
        });

        if let Some(temp) = input.temperature {
            body["temperature"] = json!(temp);
        }

        if provider_id == "zai" {
            if !crate::ZAI_MODELS_PAY_AS_YOU_GO.contains(&model) {
                return Err(format!(
                    "Unsupported model '{}' for provider 'zai'. Allowed models: {}",
                    model,
                    crate::ZAI_MODELS_PAY_AS_YOU_GO.join(", ")
                ));
            }
            if model == "glm-5.2" && profile.thinking_mode.as_deref() == Some("normal") {
                body["thinking"] = json!({"type": "disabled"});
            } else {
                if profile.thinking_mode.as_deref() == Some("normal") {
                    return Err(format!("Model '{}' does not support disabling thinking", model));
                }
                body["thinking"] = json!({"type": "enabled"});
                let effort = profile.reasoning_effort.as_deref().unwrap_or(crate::ZAI_DEFAULT_REASONING_EFFORT);
                if !crate::ZAI_REASONING_EFFORTS.contains(&effort) {
                    return Err(format!(
                        "Unsupported reasoning effort '{}' for GLM model. Allowed: {}",
                        effort,
                        crate::ZAI_REASONING_EFFORTS.join(", ")
                    ));
                }
                body["reasoning_effort"] = json!(effort);
            }
        } else {
            if let Some(ref tm) = profile.thinking_mode {
                if tm == "thinking" {
                    body["thinking"] = json!({"type": "enabled"});
                } else if tm == "normal" {
                    body["thinking"] = json!({"type": "disabled"});
                }
            }
            if let Some(ref re) = profile.reasoning_effort {
                body["reasoning_effort"] = json!(re);
            }
        }

        let start = Instant::now();
        let mut req = self
            .client
            .post(&endpoint)
            .header("Content-Type", "application/json");

        if provider_id == "zai" {
            req = req
                .header("x-api-key", api_key.trim())
                .header("anthropic-version", "2023-06-01");
        } else {
            req = req.header("Authorization", format!("Bearer {}", api_key.trim()));
        }

        if provider_id == "openrouter" {
            req = req
                .header("HTTP-Referer", "https://github.com/soheidon/poverello-coding-tools")
                .header("X-Title", "Poverello Coding Tools Orchestrator");
        }

        let cancel_fut = async {
            if let Some(token) = cancel_token {
                token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };

        // Wrap the entire lifecycle (send + headers + body download + parse) in timeout and cancellation
        let request_fut = async {
            let res = req.json(&body).send().await.map_err(|e| {
                format!(
                    "HTTP request to '{}' ({}) failed: {}",
                    provider_id, endpoint, e
                )
            })?;

            let status = res.status();
            let raw_text = res
                .text()
                .await
                .map_err(|e| format!("Failed to read response body: {}", e))?;

            if !status.is_success() {
                let redacted_err = self.redactor.redact_secrets(&raw_text);
                return Err(format!(
                    "Provider '{}' returned HTTP {}: {}",
                    provider_id,
                    status.as_u16(),
                    redacted_err
                ));
            }

            let parsed: Value = serde_json::from_str(&raw_text).map_err(|e| {
                format!(
                    "Failed to parse JSON response from '{}': {}",
                    provider_id, e
                )
            })?;

            let content = extract_content_from_response(&parsed, provider_id).ok_or_else(|| {
                "Could not extract message content from provider response".to_string()
            })?;

            let tokens_used = parsed.get("usage").and_then(|u| {
                if let Some(t) = u.get("total_tokens").and_then(|v| v.as_u64()) {
                    Some(t)
                } else {
                    let inp = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let out = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    if inp > 0 || out > 0 {
                        Some(inp + out)
                    } else {
                        None
                    }
                }
            });

            let duration_ms = start.elapsed().as_millis() as u64;

            Ok(AdapterExecutionOutput {
                content: self.redactor.redact_secrets(&content),
                raw_json: Some(self.redactor.redact_secrets(&raw_text)),
                tokens_used,
                model_used: model.to_string(),
                duration_ms,
            })
        };

        tokio::select! {
            _ = tokio::time::sleep(HTTP_INFERENCE_SAFETY_TIMEOUT) => {
                Err(format!("Inference request to provider '{}' timed out after 180 seconds.", provider_id))
            }
            _ = cancel_fut => {
                Err("Inference request cancelled by user.".to_string())
            }
            res = request_fut => {
                res
            }
        }
    }
}

pub(crate) fn resolve_provider_endpoint_and_env(provider_id: &str) -> Result<(String, String), String> {
    match provider_id {
        "mimo" => Ok((
            "https://api.xiaomimimo.com/v1/chat/completions".to_string(),
            "MIMO_API_KEY".to_string(),
        )),
        "kimi" => Ok((
            "https://api.moonshot.cn/v1/chat/completions".to_string(),
            "MOONSHOT_API_KEY".to_string(),
        )),
        "kimi_code" => Ok((
            "https://api.kimi.com/coding/v1/chat/completions".to_string(),
            "KIMI_CODE_API_KEY".to_string(),
        )),
        "minimax" => Ok((
            "https://api.minimax.chat/v1/text/chatcompletion_v2".to_string(),
            "MINIMAX_API_KEY".to_string(),
        )),
        "openrouter" => Ok((
            "https://openrouter.ai/api/v1/chat/completions".to_string(),
            "OPENROUTER_API_KEY".to_string(),
        )),
        "deepseek" => Ok((
            "https://api.deepseek.com/chat/completions".to_string(),
            "DEEPSEEK_API_KEY".to_string(),
        )),
        "zai" => Ok((
            "https://api.z.ai/api/anthropic/v1/messages".to_string(),
            "ZAI_API_KEY".to_string(),
        )),
        _ => Err(format!("Unknown provider ID '{}'", provider_id)),
    }
}

fn extract_content_from_response(json: &Value, provider_id: &str) -> Option<String> {
    if let Some(content) = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|val| val.as_str())
    {
        return Some(content.to_string());
    }

    if let Some(content_arr) = json.get("content").and_then(|c| c.as_array()) {
        if content_arr.is_empty() {
            return None;
        }

        // If stop_reason is explicitly "refusal", fail closed.
        if json.get("stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
            return None;
        }

        let mut texts = Vec::new();
        let mut has_thinking = false;

        // Strictly validate all content blocks before accepting any output.
        // In direct provider planning/orchestration responses, only text and thinking blocks are permitted.
        // Prohibited blocks: tool_use, server_tool_use, tool_result, server_tool_result,
        // refusal, unknown block types, or malformed text blocks.
        // If any prohibited block is present, the entire response fails closed (returns None),
        // even if valid text blocks are also present.
        for block in content_arr {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        texts.push(t);
                    } else {
                        // Malformed text block (missing "text" string)
                        return None;
                    }
                }
                Some("thinking") | Some("redacted_thinking") => {
                    has_thinking = true;
                }
                _ => {
                    // tool_use, refusal, unknown, or malformed: strictly reject
                    return None;
                }
            }
        }

        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }

        // Legitimate thinking-only response rescue:
        // For GLM (`zai` direct provider), when max_tokens is reached during thinking,
        // Z.ai returns a thinking-only block (type == "thinking") without any text blocks.
        // We rescue this legitimate thinking-only response ONLY when stop_reason is explicitly "max_tokens".
        // For other stop reasons (e.g. "end_turn", "stop_sequence", or missing), or for non-zai providers,
        // this must NOT be rescued and must return None.
        if provider_id == "zai"
            && has_thinking
            && json.get("stop_reason").and_then(|s| s.as_str()) == Some("max_tokens")
        {
            return Some(String::new());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::types::{AgentRole, ExecutionAdapterType, OrchestratorProfile};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::mpsc;

    fn receive_request() -> (String, mpsc::Receiver<Option<Value>>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/chat", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => {
                        connection
                            .0
                            .set_nonblocking(false)
                            .expect("failed to restore blocking mode on accepted test stream");
                        break connection;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            let _ = tx.send(None);
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("HTTP test listener failed: {error}"),
                }
            };
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8_lossy(&request);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap();
            let mut body = vec![0_u8; content_length];
            stream.read_exact(&mut body).unwrap();
            tx.send(Some(serde_json::from_slice(&body).unwrap())).unwrap();
            let response = r#"{"choices":[{"message":{"content":"sk-testsecret123456789"}}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });
        (endpoint, rx, handle)
    }

    fn input(provider_id: Option<&str>) -> AdapterExecutionInput {
        AdapterExecutionInput {
            role: AgentRole::Planner,
            profile: OrchestratorProfile {
                id: "test".into(),
                display_name: "Test".into(),
                adapter: ExecutionAdapterType::Provider,
                capabilities: vec![],
                provider_id: provider_id.map(str::to_owned),
                provider_profile_id: None,
                model: Some("test-model".into()),
                thinking_mode: None,
                reasoning_effort: None,
                ollama_model: None,
                ollama_endpoint: None,
                executable: None,
                args: None,
                external_mcp_server: None,
                mcp_tool: None,
                context_window_tokens: Some(8192),
            },
            system_prompt: "system sk-testsecret123456789 unchanged".into(),
            user_prompt: "user sk-testsecret123456789 unchanged".into(),
            project_path: PathBuf::from("."),
            temperature: None,
        }
    }

    #[tokio::test]
    async fn provider_receives_original_prompts_while_output_is_redacted() {
        let (endpoint, rx, server) = receive_request();
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };
        let output = adapter.execute(&input(Some("deepseek")), None).await.unwrap();
        let body = rx.recv().unwrap().unwrap();
        server.join().unwrap();

        assert_eq!(body["messages"][0]["content"], "system sk-testsecret123456789 unchanged");
        assert_eq!(body["messages"][1]["content"], "user sk-testsecret123456789 unchanged");
        assert!(output.content.contains("[REDACTED_SECRET]"));
        assert!(output.raw_json.unwrap().contains("[REDACTED_SECRET]"));
    }

    #[tokio::test]
    async fn missing_or_unknown_provider_id_fails_before_http_request() {
        let (endpoint, rx, server) = receive_request();
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };

        let missing = adapter.execute(&input(None), None).await.unwrap_err();
        assert!(missing.contains("missing required provider_id"));
        let unknown = adapter
            .execute(&input(Some("unknown-provider")), None)
            .await
            .unwrap_err();
        assert!(unknown.contains("Unsupported provider ID"));

        drop(adapter);
        assert_eq!(rx.recv().unwrap(), None);
        server.join().unwrap();
    }

    fn receive_request_custom(
        custom_response: Option<&str>,
    ) -> (
        String,
        mpsc::Receiver<Option<(String, Value)>>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/chat", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        let resp_str = custom_response
            .unwrap_or(r#"{"choices":[{"message":{"content":"sk-testsecret123456789"}}]}"#)
            .to_string();
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => {
                        connection
                            .0
                            .set_nonblocking(false)
                            .expect("failed to restore blocking mode on accepted test stream");
                        break connection;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            let _ = tx.send(None);
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("HTTP test listener failed: {error}"),
                }
            };
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8_lossy(&request).to_string();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap();
            let mut body = vec![0_u8; content_length];
            stream.read_exact(&mut body).unwrap();
            tx.send(Some((headers, serde_json::from_slice(&body).unwrap())))
                .unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                resp_str.len(),
                resp_str
            )
            .unwrap();
        });
        (endpoint, rx, handle)
    }

    #[tokio::test]
    async fn zai_provider_sends_default_glm53_with_thinking_and_reasoning_effort() {
        let (endpoint, rx, server) = receive_request_custom(None);
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };
        let mut in_data = input(Some("zai"));
        in_data.profile.model = None; // Should resolve to glm-5.3
        let output = adapter.execute(&in_data, None).await.unwrap();
        let (headers, body) = rx.recv().unwrap().unwrap();
        server.join().unwrap();

        assert_eq!(body["model"], "glm-5.3");
        assert_eq!(body["thinking"], json!({"type": "enabled"}));
        assert_eq!(body["reasoning_effort"], "max");
        assert_eq!(output.model_used, "glm-5.3");

        // Verify headers: x-api-key, anthropic-version, no Authorization
        assert!(headers.contains("x-api-key: test-key"));
        assert!(headers.contains("anthropic-version: 2023-06-01"));
        assert!(!headers.to_lowercase().contains("authorization"));
    }

    #[tokio::test]
    async fn zai_provider_handles_thinking_only_max_tokens_response() {
        let fixture = json!({
            "id": "msg_flash_1",
            "type": "message",
            "role": "assistant",
            "model": "glm-5.3-flash",
            "content": [
                {
                    "type": "thinking",
                    "thinking": "analyzing request deeply..."
                }
            ],
            "stop_reason": "max_tokens",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 64
            }
        })
        .to_string();

        let (endpoint, _rx, server) = receive_request_custom(Some(&fixture));
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };
        let mut in_data = input(Some("zai"));
        in_data.profile.model = Some("glm-5.3-flash".into());
        let output = adapter.execute(&in_data, None).await.unwrap();
        server.join().unwrap();

        // Transport layer must succeed, preserve empty content for text,
        // keep total tokens used (10 + 64 = 74), and preserve full raw_json.
        assert_eq!(output.content, "");
        assert_eq!(output.tokens_used, Some(74));
        assert_eq!(output.model_used, "glm-5.3-flash");
        let raw = output.raw_json.expect("raw_json must be preserved");
        assert!(raw.contains("analyzing request deeply..."));
        assert!(raw.contains("max_tokens"));
    }

    #[tokio::test]
    async fn zai_provider_glm52_allows_disabling_thinking_without_effort() {
        let (endpoint, rx, server) = receive_request();
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };
        let mut in_data = input(Some("zai"));
        in_data.profile.model = Some("glm-5.2".into());
        in_data.profile.thinking_mode = Some("normal".into());
        let output = adapter.execute(&in_data, None).await.unwrap();
        let body = rx.recv().unwrap().unwrap();
        server.join().unwrap();

        assert_eq!(body["model"], "glm-5.2");
        assert_eq!(body["thinking"], json!({"type": "disabled"}));
        assert!(body.get("reasoning_effort").is_none());
        assert_eq!(output.model_used, "glm-5.2");
    }

    #[tokio::test]
    async fn zai_provider_glm53_refuses_disabling_thinking() {
        let (endpoint, rx, server) = receive_request();
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };
        let mut in_data = input(Some("zai"));
        in_data.profile.model = Some("glm-5.3".into());
        in_data.profile.thinking_mode = Some("normal".into());
        let err = adapter.execute(&in_data, None).await.unwrap_err();
        assert!(err.contains("does not support disabling thinking"), "{err}");

        drop(adapter);
        assert_eq!(rx.recv().unwrap(), None);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn zai_provider_undocumented_model_refuses_before_http_request() {
        let (endpoint, rx, server) = receive_request();
        let adapter = ProviderAdapter {
            endpoint_override: Some(endpoint),
            api_key_override: Some("test-key".into()),
            ..ProviderAdapter::new()
        };
        let mut in_data = input(Some("zai"));
        in_data.profile.model = Some("glm-undocumented".into());
        let err = adapter.execute(&in_data, None).await.unwrap_err();
        assert!(err.contains("Unsupported model 'glm-undocumented' for provider 'zai'"), "{err}");

        drop(adapter);
        assert_eq!(rx.recv().unwrap(), None);
        server.join().unwrap();
    }

    #[test]
    fn test_extract_content_from_response_scenarios() {
        // 1. Text block present -> returns text for any provider
        let with_text = json!({
            "content": [
                { "type": "thinking", "thinking": "reasoning..." },
                { "type": "text", "text": "final answer" }
            ]
        });
        assert_eq!(extract_content_from_response(&with_text, "zai"), Some("final answer".to_string()));
        assert_eq!(extract_content_from_response(&with_text, "anthropic"), Some("final answer".to_string()));

        // 1b. Multiple text blocks -> joined with newline delimiter
        let with_multiple_texts = json!({
            "content": [
                { "type": "thinking", "thinking": "reasoning..." },
                { "type": "text", "text": "part 1" },
                { "type": "text", "text": "part 2" }
            ]
        });
        assert_eq!(extract_content_from_response(&with_multiple_texts, "zai"), Some("part 1\npart 2".to_string()));
        assert_eq!(extract_content_from_response(&with_multiple_texts, "anthropic"), Some("part 1\npart 2".to_string()));

        // 2. OpenAI choices format -> returns content for any provider
        let openai_format = json!({
            "choices": [{ "message": { "content": "openai text" } }]
        });
        assert_eq!(extract_content_from_response(&openai_format, "deepseek"), Some("openai text".to_string()));

        // 3. Z.ai thinking-only (legitimate) -> returns empty string ONLY with stop_reason == "max_tokens"
        let zai_thinking_only = json!({
            "content": [
                { "type": "thinking", "thinking": "only thinking here" }
            ],
            "stop_reason": "max_tokens"
        });
        assert_eq!(extract_content_from_response(&zai_thinking_only, "zai"), Some("".to_string()));

        // 3b. Z.ai thinking-only with other stop_reasons -> returns None (MUST NOT rescue)
        let zai_thinking_end_turn = json!({
            "content": [
                { "type": "thinking", "thinking": "only thinking here" }
            ],
            "stop_reason": "end_turn"
        });
        assert_eq!(extract_content_from_response(&zai_thinking_end_turn, "zai"), None);

        let zai_thinking_stop_sequence = json!({
            "content": [
                { "type": "thinking", "thinking": "only thinking here" }
            ],
            "stop_reason": "stop_sequence"
        });
        assert_eq!(extract_content_from_response(&zai_thinking_stop_sequence, "zai"), None);

        let zai_thinking_no_stop_reason = json!({
            "content": [
                { "type": "thinking", "thinking": "only thinking here" }
            ]
        });
        assert_eq!(extract_content_from_response(&zai_thinking_no_stop_reason, "zai"), None);

        // 4. Non-zai provider thinking-only -> returns None (must NOT rescue)
        assert_eq!(extract_content_from_response(&zai_thinking_only, "anthropic"), None);
        assert_eq!(extract_content_from_response(&zai_thinking_only, "openrouter"), None);
        assert_eq!(extract_content_from_response(&zai_thinking_only, "deepseek"), None);

        // 5. Z.ai tool-only -> returns None
        let zai_tool_only = json!({
            "content": [
                { "type": "tool_use", "id": "t1", "name": "read_file", "input": {} }
            ]
        });
        assert_eq!(extract_content_from_response(&zai_tool_only, "zai"), None);
        assert_eq!(extract_content_from_response(&zai_tool_only, "anthropic"), None);

        // 6. Z.ai refusal -> returns None
        let zai_refusal = json!({
            "content": [
                { "type": "refusal", "message": "I cannot answer this" }
            ]
        });
        assert_eq!(extract_content_from_response(&zai_refusal, "zai"), None);
        assert_eq!(extract_content_from_response(&zai_refusal, "anthropic"), None);

        // 7. Z.ai unknown/malformed content block -> returns None
        let zai_unknown = json!({
            "content": [
                { "type": "unknown_type_foo" }
            ]
        });
        assert_eq!(extract_content_from_response(&zai_unknown, "zai"), None);
        assert_eq!(extract_content_from_response(&zai_unknown, "anthropic"), None);

        // 8. Empty content array -> returns None
        let empty_content = json!({ "content": [] });
        assert_eq!(extract_content_from_response(&empty_content, "zai"), None);
        assert_eq!(extract_content_from_response(&empty_content, "anthropic"), None);

        // 9. Mixed thinking and tool_use without text -> returns None (disallowed)
        let mixed_disallowed = json!({
            "content": [
                { "type": "thinking", "thinking": "plan" },
                { "type": "tool_use", "id": "t1", "name": "exec", "input": {} }
            ]
        });
        assert_eq!(extract_content_from_response(&mixed_disallowed, "zai"), None);

        // 10. Text mixed with prohibited blocks -> returns None (MUST NOT discard prohibited block and return text)
        let text_with_tool = json!({
            "content": [
                { "type": "text", "text": "Here is the plan" },
                { "type": "tool_use", "id": "t1", "name": "exec", "input": {} }
            ]
        });
        assert_eq!(extract_content_from_response(&text_with_tool, "zai"), None);
        assert_eq!(extract_content_from_response(&text_with_tool, "anthropic"), None);

        let text_with_refusal = json!({
            "content": [
                { "type": "text", "text": "partial text" },
                { "type": "refusal", "message": "cannot proceed" }
            ]
        });
        assert_eq!(extract_content_from_response(&text_with_refusal, "zai"), None);
        assert_eq!(extract_content_from_response(&text_with_refusal, "anthropic"), None);

        let text_with_unknown = json!({
            "content": [
                { "type": "text", "text": "valid text" },
                { "type": "unrecognized_third_party_block" }
            ]
        });
        assert_eq!(extract_content_from_response(&text_with_unknown, "zai"), None);
        assert_eq!(extract_content_from_response(&text_with_unknown, "anthropic"), None);

        let text_with_malformed = json!({
            "content": [
                { "type": "text", "text": "valid text" },
                { "type": "text" } // missing "text" string
            ]
        });
        assert_eq!(extract_content_from_response(&text_with_malformed, "zai"), None);
        assert_eq!(extract_content_from_response(&text_with_malformed, "anthropic"), None);

        let text_with_refusal_stop_reason = json!({
            "content": [
                { "type": "text", "text": "some text" }
            ],
            "stop_reason": "refusal"
        });
        assert_eq!(extract_content_from_response(&text_with_refusal_stop_reason, "zai"), None);
        assert_eq!(extract_content_from_response(&text_with_refusal_stop_reason, "anthropic"), None);
    }
}
