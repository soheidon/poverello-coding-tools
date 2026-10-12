//! DynamicBridgeProvider: resolves target provider, profile, model, and reasoning
//! settings from Poverello's `config.json` and dispatches planning requests.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use super::{PlanResponse, PlannerProvider, ProviderError};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TOKENS: u32 = 8192;
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
#[cfg(test)]
const GLM_MODEL: &str = "glm-5.3";
const GLM_MODELS: &[&str] = &["glm-5.3", "glm-5.3-flash", "glm-5.3-flashx", "glm-5.2"];
const ZAI_ENDPOINT: &str = "https://api.z.ai/api/anthropic/v1/messages";

/// Resolves the APPDATA configuration path for Poverello.
pub fn resolve_anthro_bridge_config_path() -> PathBuf {
    let appdata = env::var("APPDATA")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    let channel_dir = match option_env!("ANTHRO_BRIDGE_CHANNEL") {
        Some("stable") => "Poverello",
        Some("dev") | None => "Poverello Dev",
        Some(_) => "Poverello Dev",
    };

    let p = PathBuf::from(appdata).join(channel_dir).join("config.json");
    if p.exists() {
        return p;
    }

    // Fallback: check stable directory if dev does not exist
    let p_stable = PathBuf::from(env::var("APPDATA").unwrap_or_default())
        .join("Poverello")
        .join("config.json");
    if p_stable.exists() {
        return p_stable;
    }

    p
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ConfigMcpSection {
    provider: Option<String>,
    profile_id: Option<String>,
    model: Option<String>,
    thinking_mode: Option<String>,
    reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderEntry {
    upstream_url: String,
    api_key_env: String,
    #[serde(default)]
    default_model: String,
    #[serde(default)]
    profiles: Vec<ProfileEntry>,
    #[serde(default)]
    models: std::collections::HashMap<String, ModelEntryRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProfileEntry {
    id: String,
    #[allow(dead_code)]
    display_name: String,
    #[serde(default)]
    models: std::collections::HashMap<String, ModelEntryRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelEntryRaw {
    upstream_model: String,
    thinking_mode: Option<String>,
    reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BridgeConfig {
    #[serde(default)]
    active_provider: Option<String>,
    #[serde(default)]
    active_openrouter_profile_id: Option<String>,
    #[serde(default)]
    providers: std::collections::HashMap<String, ProviderEntry>,
    #[serde(default)]
    mcp: Option<ConfigMcpSection>,
}

#[derive(Debug, Clone)]
pub struct ResolvedMcpTarget {
    pub provider_id: String,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub thinking_mode: Option<String>,
    pub reasoning_effort: Option<String>,
    pub is_openrouter: bool,
}

pub fn resolve_mcp_target(
    config_path: &std::path::Path,
) -> Result<ResolvedMcpTarget, ProviderError> {
    resolve_mcp_target_with_zai_endpoint(config_path, ZAI_ENDPOINT)
}

fn resolve_mcp_target_with_zai_endpoint(
    config_path: &std::path::Path,
    zai_endpoint: &str,
) -> Result<ResolvedMcpTarget, ProviderError> {
    if !config_path.exists() {
        // Fallback to legacy environment variable if config.json not yet created
        let api_key = env::var("DEEPSEEK_API_KEY").unwrap_or_default();
        if api_key.trim().is_empty() {
            return Err(ProviderError::Config(format!(
                "Poverello config not found at {} and DEEPSEEK_API_KEY is not set",
                config_path.display()
            )));
        }
        return Ok(ResolvedMcpTarget {
            provider_id: "deepseek".to_string(),
            endpoint: "https://api.deepseek.com/anthropic/v1/messages".to_string(),
            api_key,
            model: "deepseek-v4-pro".to_string(),
            thinking_mode: Some("thinking".to_string()),
            reasoning_effort: Some("high".to_string()),
            is_openrouter: false,
        });
    }

    let content = std::fs::read_to_string(config_path).map_err(|e| {
        ProviderError::Config(format!("Failed to read {}: {e}", config_path.display()))
    })?;
    let cfg: BridgeConfig = serde_json::from_str(&content).map_err(|e| {
        ProviderError::Config(format!("Failed to parse {}: {e}", config_path.display()))
    })?;

    let mcp = cfg.mcp.unwrap_or_default();
    let provider_id = mcp
        .provider
        .clone()
        .or_else(|| cfg.active_provider.clone())
        .unwrap_or_else(|| "deepseek".to_string());

    if provider_id == "zai" && mcp.provider.as_deref() != Some("zai") {
        return Err(ProviderError::Config(
            "GLM MCP requires an explicit mcp.provider=zai selection".into(),
        ));
    }

    let provider = cfg.providers.get(&provider_id).ok_or_else(|| {
        ProviderError::Config(format!(
            "Provider '{}' not configured in config.json",
            provider_id
        ))
    })?;

    let api_key = env::var(&provider.api_key_env).unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(ProviderError::Config(format!(
            "API key environment variable '{}' for provider '{}' is not set or is empty",
            provider.api_key_env, provider_id
        )));
    }

    let is_openrouter = provider_id == "openrouter";

    let (model, tm, re) = if is_openrouter {
        let active_prof = if let Some(ref pid) = mcp.profile_id {
            provider.profiles.iter().find(|p| p.id == *pid)
        } else if let Some(ref pid) = cfg.active_openrouter_profile_id {
            provider.profiles.iter().find(|p| p.id == *pid)
        } else {
            provider.profiles.first()
        };

        let m_name = mcp
            .model
            .clone()
            .or_else(|| {
                active_prof.and_then(|p| {
                    p.models
                        .get("claude-opus-5")
                        .map(|m| m.upstream_model.clone())
                })
            })
            .unwrap_or_else(|| "deepseek/deepseek-r1".to_string());

        let tm = mcp.thinking_mode.clone().or_else(|| {
            active_prof.and_then(|p| {
                p.models
                    .get("claude-opus-5")
                    .and_then(|m| m.thinking_mode.clone())
            })
        });

        let re = mcp.reasoning_effort.clone().or_else(|| {
            active_prof.and_then(|p| {
                p.models
                    .get("claude-opus-5")
                    .and_then(|m| m.reasoning_effort.clone())
            })
        });

        (m_name, tm, re)
    } else {
        let m_name = mcp
            .model
            .clone()
            .or_else(|| {
                provider
                    .models
                    .get("claude-opus-5")
                    .map(|m| m.upstream_model.clone())
            })
            .unwrap_or_else(|| provider.default_model.clone());

        let tm = mcp.thinking_mode.clone().or_else(|| {
            provider
                .models
                .get("claude-opus-5")
                .and_then(|m| m.thinking_mode.clone())
        });

        let re = mcp.reasoning_effort.clone().or_else(|| {
            provider
                .models
                .get("claude-opus-5")
                .and_then(|m| m.reasoning_effort.clone())
        });

        (m_name, tm, re)
    };

    let endpoint = if provider_id == "anthropic" {
        // Do not let a configurable base URL redirect the Anthropic API key.
        ANTHROPIC_ENDPOINT.to_string()
    } else if provider_id == "zai" {
        // Z.ai is reached over its Anthropic Messages endpoint only.
        zai_endpoint.to_string()
    } else {
        let base_url = provider.upstream_url.trim_end_matches('/');
        if base_url.ends_with("/v1/messages") {
            base_url.to_string()
        } else if base_url.ends_with("/v1") {
            format!("{base_url}/messages")
        } else {
            format!("{base_url}/v1/messages")
        }
    };

    if provider_id == "zai" && !GLM_MODELS.contains(&model.as_str()) {
        return Err(ProviderError::Config(format!(
            "Unsupported model '{model}' for provider 'zai'. Allowed models: {}",
            GLM_MODELS.join(", ")
        )));
    }

    Ok(ResolvedMcpTarget {
        provider_id,
        endpoint,
        api_key,
        model,
        thinking_mode: tm,
        reasoning_effort: re,
        is_openrouter,
    })
}

/// Dynamic MCP planner provider that reads `config.json` on each call (or uses injectable target).
#[derive(Debug, Clone)]
pub struct DynamicBridgeProvider {
    client: reqwest::Client,
    config_path: Option<PathBuf>,
    override_target: Option<ResolvedMcpTarget>,
    timeout: Duration,
    #[cfg(test)]
    test_zai_endpoint: Option<String>,
}

impl DynamicBridgeProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            config_path: Some(resolve_anthro_bridge_config_path()),
            override_target: None,
            timeout: DEFAULT_TIMEOUT,
            #[cfg(test)]
            test_zai_endpoint: None,
        }
    }

    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    pub fn with_target(mut self, target: ResolvedMcpTarget) -> Self {
        self.override_target = Some(target);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn sanitize_provider_message(&self, message: String, api_key: &str) -> String {
        if api_key.trim().is_empty() {
            return sanitize(message);
        }
        let redacted = message
            .replace(&format!("Bearer {}", api_key), "[REDACTED]")
            .replace(api_key, "[REDACTED]");
        sanitize(redacted)
    }

    #[cfg(test)]
    fn with_test_zai_endpoint(mut self, endpoint: String) -> Self {
        self.test_zai_endpoint = Some(endpoint);
        self
    }
}

impl Default for DynamicBridgeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PlannerProvider for DynamicBridgeProvider {
    async fn plan(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<PlanResponse, ProviderError> {
        let target = if let Some(ref t) = self.override_target {
            t.clone()
        } else if let Some(ref p) = self.config_path {
            #[cfg(test)]
            let resolved = if let Some(endpoint) = &self.test_zai_endpoint {
                resolve_mcp_target_with_zai_endpoint(p, endpoint)?
            } else {
                resolve_mcp_target(p)?
            };
            #[cfg(not(test))]
            let resolved = resolve_mcp_target(p)?;
            resolved
        } else {
            return Err(ProviderError::Config("No configuration path set".into()));
        };

        if target.api_key.trim().is_empty() {
            return Err(ProviderError::Config(format!(
                "API key for provider '{}' is not set",
                target.provider_id
            )));
        }

        let is_thinking = target.thinking_mode.as_deref() == Some("thinking")
            || target.thinking_mode.as_deref() == Some("thinking_only");

        let is_glm = target.provider_id == "zai";
        if is_glm {
            if !GLM_MODELS.contains(&target.model.as_str()) {
                return Err(ProviderError::Config(format!(
                    "Unsupported model '{}' for provider 'zai'. Allowed models: {}",
                    target.model,
                    GLM_MODELS.join(", ")
                )));
            }
            if target.model == "glm-5.2"
                && matches!(target.thinking_mode.as_deref(), Some("normal" | "disabled"))
            {
                // GLM-5.2 supports disabling thinking
            } else {
                if matches!(target.thinking_mode.as_deref(), Some("normal" | "disabled")) {
                    return Err(ProviderError::Config(format!(
                        "Model '{}' does not support disabling thinking",
                        target.model
                    )));
                }
                if let Some(effort) = target.reasoning_effort.as_deref() {
                    if !matches!(effort, "low" | "high" | "max") {
                        return Err(ProviderError::Config(
                            "GLM reasoning effort must be low, high, or max".into(),
                        ));
                    }
                }
            }
        }

        let mut req_body = serde_json::json!({
            "model": target.model,
            "max_tokens": MAX_TOKENS,
            "system": system_prompt,
            "messages": [ { "role": "user", "content": user_prompt } ],
            "stream": false
        });

        match target.provider_id.as_str() {
            "deepseek" => {
                if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                    if let Some(ref effort) = target.reasoning_effort {
                        if !effort.trim().is_empty() {
                            req_body["output_config"] = serde_json::json!({ "effort": effort });
                        }
                    }
                }
            }
            "anthropic" => {
                let (thinking, effort) = anthropic_thinking_fields(
                    &target.model,
                    target.thinking_mode.as_deref(),
                    target.reasoning_effort.as_deref(),
                )?;
                if let Some(thinking) = thinking {
                    req_body["thinking"] = thinking;
                }
                if let Some(effort) = effort {
                    req_body["output_config"] = serde_json::json!({ "effort": effort });
                }
            }
            "zai" => {
                if target.model == "glm-5.2"
                    && matches!(target.thinking_mode.as_deref(), Some("normal" | "disabled"))
                {
                    req_body["thinking"] = serde_json::json!({ "type": "disabled" });
                } else {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                    req_body["reasoning_effort"] =
                        serde_json::json!(target.reasoning_effort.as_deref().unwrap_or("max"));
                }
            }
            "mimo" => {
                if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                } else {
                    req_body["thinking"] = serde_json::json!({ "type": "disabled" });
                }
            }
            "minimax" => {
                if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                }
                // Do NOT send output_config to MiniMax
            }
            "kimi" => {
                let is_k3 = target.model.contains("k3");
                if is_k3 {
                    // K3: do NOT send thinking parameter; send reasoning_effort directly
                    let effort = target
                        .reasoning_effort
                        .as_deref()
                        .filter(|e| !e.trim().is_empty())
                        .unwrap_or("max");
                    req_body["reasoning_effort"] = serde_json::json!(effort);
                } else if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                }
            }
            "kimi-code" => {
                if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                }
            }
            "openrouter" => {
                let is_openai_astra = matches!(
                    target.model.as_str(),
                    "openai/gpt-6-astra" | "openai/gpt-astra-latest"
                );
                let is_openai_astra_pro = target.model == "openai/gpt-6-astra-pro";
                let is_openai_gpt56 = matches!(
                    target.model.as_str(),
                    "openai/gpt-5.6-sol"
                        | "openai/gpt-5.6-sol-pro"
                        | "openai/gpt-5.6-terra"
                        | "openai/gpt-5.6-terra-pro"
                        | "openai/gpt-5.6-luna"
                        | "openai/gpt-5.6-luna-pro"
                );
                let is_poolside =
                    target.model.contains("laguna") || target.model.contains("poolside");
                let is_deepseek_v4_1 = target.model == "deepseek/deepseek-v4.1-flash";
                let is_deepseek_fixed_v4 = matches!(
                    target.model.as_str(),
                    "deepseek/deepseek-v4-flash-0731" | "deepseek/deepseek-v4-pro-0813"
                );
                if is_openai_astra {
                    let effort = match target.reasoning_effort.as_deref() {
                        Some(e @ ("low" | "medium" | "high" | "xhigh" | "max")) => e,
                        _ => "high",
                    };
                    req_body["reasoning"] = serde_json::json!({ "effort": effort });
                } else if is_openai_astra_pro {
                    // Dedicated Pro endpoint: no effort parameter sent, no Anthropic thinking envelope.
                } else if is_openai_gpt56 {
                    if is_thinking {
                        let effort = match target.reasoning_effort.as_deref() {
                            Some(e @ ("low" | "medium" | "high" | "xhigh" | "max")) => e,
                            _ => "medium",
                        };
                        req_body["reasoning"] = serde_json::json!({ "effort": effort });
                    } else if target.thinking_mode.as_deref() == Some("normal") {
                        req_body["reasoning"] = serde_json::json!({ "effort": "none" });
                    }
                } else if is_deepseek_v4_1 {
                    if is_thinking {
                        let effort = match target.reasoning_effort.as_deref() {
                            Some("low") => "low",
                            Some("max") | Some("xhigh") => "max",
                            _ => "high",
                        };
                        req_body["reasoning"] = serde_json::json!({ "effort": effort });
                    } else if target.thinking_mode.as_deref() == Some("normal") {
                        req_body["reasoning"] = serde_json::json!({ "enabled": false });
                    }
                } else if is_deepseek_fixed_v4 {
                    if is_thinking {
                        let effort = match target.reasoning_effort.as_deref() {
                            Some("max") | Some("xhigh") => "max",
                            _ => "high",
                        };
                        req_body["reasoning"] = serde_json::json!({ "effort": effort });
                    } else if target.thinking_mode.as_deref() == Some("normal") {
                        req_body["reasoning"] = serde_json::json!({ "enabled": false });
                    }
                } else if is_poolside {
                    if is_thinking {
                        if target.reasoning_effort.as_deref() == Some("max") {
                            req_body["reasoning"] = serde_json::json!({ "effort": "max" });
                        } else {
                            req_body["reasoning"] = serde_json::json!({ "enabled": true });
                        }
                    } else {
                        req_body["reasoning"] = serde_json::json!({ "enabled": false });
                    }
                } else if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                    if let Some(ref effort) = target.reasoning_effort {
                        if !effort.trim().is_empty() {
                            req_body["reasoning"] = serde_json::json!({ "effort": effort });
                        }
                    }
                }
            }
            _ => {
                // Unknown/fallback provider
                if is_thinking {
                    req_body["thinking"] = serde_json::json!({ "type": "enabled" });
                }
            }
        }

        let mut request = self
            .client
            .post(&target.endpoint)
            .header("Content-Type", "application/json");
        if target.provider_id == "anthropic" || target.provider_id == "zai" {
            request = request
                .header("x-api-key", &target.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION);
        } else {
            request = request.header("Authorization", format!("Bearer {}", target.api_key));
        }
        let response = request
            .json(&req_body)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(classify_request_error)?;

        let status = response.status();
        if !status.is_success() {
            let message = self.sanitize_provider_message(
                response.text().await.unwrap_or_default(),
                &target.api_key,
            );
            return Err(classify_http_error(status, message));
        }

        let (text, is_max_tokens_thinking_only) = {
            let body: AnthropicMessagesResponse = response.json().await.map_err(|e| {
                ProviderError::Response(format!("failed to parse provider response: {e}"))
            })?;

            if body.stop_reason.as_deref() == Some("refusal") {
                return Err(ProviderError::Response(
                    "provider returned refusal stop_reason".into(),
                ));
            }

            let mut texts = Vec::new();
            let mut has_thinking = false;
            for block in body.content {
                match block {
                    ContentBlock::Text { text } => {
                        texts.push(text);
                    }
                    ContentBlock::Thinking | ContentBlock::RedactedThinking => {
                        has_thinking = true;
                        // Excluded from plan text output
                    }
                    ContentBlock::Refusal { message } => {
                        let reason = message
                            .filter(|m| !m.trim().is_empty())
                            .unwrap_or_else(|| "provider returned refusal block".into());
                        return Err(ProviderError::Response(reason));
                    }
                    ContentBlock::ToolUse { .. }
                    | ContentBlock::ServerToolUse { .. }
                    | ContentBlock::ToolResult { .. }
                    | ContentBlock::ServerToolResult { .. } => {
                        return Err(ProviderError::Response(
                            "provider returned tool block in plan response".into(),
                        ));
                    }
                    ContentBlock::Unknown => {
                        return Err(ProviderError::Response(
                            "provider returned unknown or invalid content block".into(),
                        ));
                    }
                }
            }

            let is_max_tokens_thinking =
                has_thinking && body.stop_reason.as_deref() == Some("max_tokens");
            (texts.join("\n"), is_max_tokens_thinking)
        };

        if text.trim().is_empty() {
            if is_max_tokens_thinking_only {
                return Ok(PlanResponse { text: String::new() });
            }
            return Err(ProviderError::Response(
                "provider returned no text content".into(),
            ));
        }

        Ok(PlanResponse { text })
    }
}

fn classify_request_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Timeout(err.to_string())
    } else {
        ProviderError::Network(err.to_string())
    }
}

fn anthropic_thinking_fields(
    model: &str,
    thinking_mode: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Result<(Option<serde_json::Value>, Option<String>), ProviderError> {
    let model_kind = if model == "claude-opus-5-5" || model.starts_with("claude-opus-5-5-") {
        "opus"
    } else if model == "claude-sonnet-5-5" || model.starts_with("claude-sonnet-5-5-") {
        "sonnet"
    } else if model == "claude-haiku-5-5" || model.starts_with("claude-haiku-5-5-") {
        "haiku"
    } else {
        return Err(ProviderError::Config(
            "Anthropic MCP currently supports only Claude Opus, Sonnet, and Haiku 5.5".into(),
        ));
    };

    let effort = match reasoning_effort {
        None | Some("") => None,
        Some(effort @ ("low" | "medium" | "high" | "xhigh" | "max")) => Some(effort.to_string()),
        Some(_) => {
            return Err(ProviderError::Config(
                "Anthropic reasoning effort must be low, medium, high, xhigh, or max".into(),
            ));
        }
    };

    let thinking = match thinking_mode {
        None => None,
        Some("thinking" | "thinking_only") => Some(serde_json::json!({ "type": "adaptive" })),
        Some("normal") if model_kind == "sonnet" => {
            if matches!(effort.as_deref(), Some("xhigh" | "max")) {
                return Err(ProviderError::Config(
                    "Sonnet 5.5 between_tools mode supports effort high or below".into(),
                ));
            }
            Some(serde_json::json!({ "type": "between_tools" }))
        }
        Some("normal" | "disabled") if model_kind == "haiku" => {
            if matches!(effort.as_deref(), Some("xhigh" | "max")) {
                return Err(ProviderError::Config(
                    "Haiku 5.5 disabled thinking supports effort high or below".into(),
                ));
            }
            Some(serde_json::json!({ "type": "disabled" }))
        }
        Some("normal" | "disabled") => {
            return Err(ProviderError::Config(
                "Thinking cannot be disabled for Anthropic Opus 5.5; Sonnet uses normal/between_tools and Haiku permits disabled".into(),
            ));
        }
        Some(_) => {
            return Err(ProviderError::Config(
                "Anthropic MCP thinking mode is unsupported".into(),
            ));
        }
    };

    Ok((thinking, effort))
}

fn classify_http_error(status: reqwest::StatusCode, message: String) -> ProviderError {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        ProviderError::Auth(message)
    } else {
        ProviderError::Http {
            status: status.as_u16(),
            message,
        }
    }
}

fn sanitize(message: String) -> String {
    const MAX_LEN: usize = 500;
    if message.chars().count() > MAX_LEN {
        let mut truncated: String = message.chars().take(MAX_LEN).collect();
        truncated.push('\u{2026}');
        truncated
    } else {
        message
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "redacted_thinking")]
    RedactedThinking,
    #[serde(rename = "refusal")]
    Refusal {
        #[serde(default)]
        message: Option<String>,
    },
    #[serde(rename = "tool_use")]
    ToolUse,
    #[serde(rename = "server_tool_use")]
    ServerToolUse,
    #[serde(rename = "tool_result")]
    ToolResult,
    #[serde(rename = "server_tool_result")]
    ServerToolResult,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod glm_m2_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    fn test_config(key_env: &str) -> String {
        serde_json::json!({
            "active_provider": "deepseek",
            "providers": {
                "zai": {
                    "upstream_url": "https://api.z.ai/api/anthropic",
                    "api_key_env": key_env,
                    "default_model": GLM_MODEL,
                    "models": {
                        "claude-opus-5": {
                            "upstream_model": GLM_MODEL,
                            "thinking_mode": "thinking",
                            "reasoning_effort": "high"
                        }
                    }
                }
            },
            "mcp": { "provider": "zai" }
        })
        .to_string()
    }

    fn write_temp_config(contents: &str) -> (PathBuf, String) {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let env_name = format!("MCP_M2_GLM_KEY_{}_{}", std::process::id(), id);
        env::set_var(&env_name, "test-only-glm-key");
        let path = env::temp_dir().join(format!(
            "mcp_m2_glm_config_{}_{}.json",
            std::process::id(),
            id
        ));
        std::fs::write(&path, contents.replace("__KEY_ENV__", &env_name)).unwrap();
        (path, env_name)
    }

    fn cleanup_temp_config(path: &std::path::Path, env_name: &str) {
        let _ = std::fs::remove_file(path);
        env::remove_var(env_name);
    }

    #[test]
    fn glm_resolves_to_the_anthropic_messages_endpoint() {
        let (path, env_name) = write_temp_config(&test_config("__KEY_ENV__"));
        let target = resolve_mcp_target(&path).unwrap();
        assert_eq!(target.endpoint, "https://api.z.ai/api/anthropic/v1/messages");
        assert!(!target.endpoint.contains("chat/completions"));
        assert_eq!(target.model, GLM_MODEL);
        cleanup_temp_config(&path, &env_name);
    }

    #[test]
    fn glm_still_requires_an_explicit_mcp_provider_selection() {
        let mut config: serde_json::Value =
            serde_json::from_str(&test_config("__KEY_ENV__")).unwrap();
        config["mcp"] = serde_json::json!({});
        config["active_provider"] = "zai".into();
        let (path, env_name) = write_temp_config(&config.to_string());
        assert!(matches!(
            resolve_mcp_target(&path),
            Err(ProviderError::Config(_))
        ));
        cleanup_temp_config(&path, &env_name);
    }

    #[tokio::test]
    async fn glm_dispatches_once_as_anthropic_messages_without_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/anthropic/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [
                    { "type": "thinking", "thinking": "hidden" },
                    { "type": "text", "text": "GLM fixture response" }
                ],
                "usage": { "input_tokens": 9, "output_tokens": 4 }
            })))
            .mount(&server)
            .await;

        let (path_config, env_name) = write_temp_config(&test_config("__KEY_ENV__"));
        let provider = DynamicBridgeProvider::new()
            .with_config_path(path_config.clone())
            .with_test_zai_endpoint(format!("{}/api/anthropic/v1/messages", server.uri()));

        let result = provider.plan("system prompt", "task").await.unwrap();
        assert_eq!(result.text, "GLM fixture response");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "no retry or fallback request");
        assert_eq!(requests[0].url.path(), "/api/anthropic/v1/messages");
        assert_eq!(
            requests[0].headers.get("x-api-key").unwrap(),
            "test-only-glm-key"
        );
        assert_eq!(
            requests[0].headers.get("anthropic-version").unwrap(),
            ANTHROPIC_VERSION
        );
        assert!(requests[0].headers.get("authorization").is_none());
        let payload: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(payload["model"], GLM_MODEL);
        assert_eq!(payload["system"], "system prompt");
        assert_eq!(payload["messages"][0]["role"], "user");
        assert_eq!(payload["messages"][0]["content"], "task");
        assert_eq!(payload["thinking"]["type"], "enabled");
        assert_eq!(payload["reasoning_effort"], "high");
        assert_eq!(payload["stream"], false);

        cleanup_temp_config(&path_config, &env_name);
    }
}
