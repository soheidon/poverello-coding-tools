// ---------------------------------------------------------------------------
// Shared route→upstream model resolution.
//
// The proxy (proxy.rs `resolve_proxy_config`) and the auto-compact resolver
// (lib.rs) must agree on which upstream model a route maps to, so both use the
// extractors in this module. The caller decides WHICH map is authoritative —
// the proxy via its typed `models`/`model_map` branch, this module's raw-JSON
// resolver via the `models`-object presence check — while `resolve_from_models`
// / `resolve_from_model_map` perform the actual lookup. Do NOT impose a new
// precedence here; it must stay faithful to what the proxy does, or route
// resolution will diverge.
// ---------------------------------------------------------------------------

/// The three canonical Gateway model routes used by Claude Code.
pub(crate) const CLAUDE_ROUTES: [&str; 3] =
    ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];

/// Additional canonical routes for the Claude 5.5 generation.
///
/// Deliberately separate from `CLAUDE_ROUTES`: the base three are the routes
/// every target must resolve for the auto-compact capacity basis, while the 5.5
/// routes exist only on targets that carry an explicit mapping (currently
/// `OpenRouter: Anthropic`). Folding them into `CLAUDE_ROUTES` would make every
/// target without a mapping report an unresolved route and lose its
/// context-window basis. Requests for these routes on a target without a
/// mapping fail closed — the proxy returns `Unknown model` before dispatch.
pub(crate) const CLAUDE_55_ROUTES: [&str; 3] =
    ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"];

/// Extract `models[route].upstream_model` from a scope (provider or profile).
///
/// Does NOT decide whether `models` is authoritative — the caller has already
/// selected it. Returns `None` when the scope has no `models` object or the
/// route is absent.
pub(crate) fn resolve_from_models(scope: &serde_json::Value, route: &str) -> Option<String> {
    scope
        .get("models")
        .and_then(|m| m.get(route))
        .and_then(|entry| entry.get("upstream_model"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Extract `model_map[route]` from a scope (provider or profile).
///
/// Does NOT decide whether `model_map` is authoritative — the caller has
/// already selected it. Returns `None` when the scope has no `model_map` or
/// the route is absent.
pub(crate) fn resolve_from_model_map(scope: &serde_json::Value, route: &str) -> Option<String> {
    scope
        .get("model_map")
        .and_then(|m| m.get(route))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Resolve the upstream model a route maps to for one provider/profile.
///
/// Mirrors proxy.rs `resolve_proxy_config` exactly:
/// - `openrouter`: the active profile's `models[route].upstream_model` ONLY —
///   the proxy skips provider/profile `model_map` entirely when profiles exist.
///   When `profile_id` is `None`, the active profile (by
///   `active_openrouter_profile_id`) is used, falling back to the first profile
///   — the proxy's transient fallback.
/// - direct provider: exclusive selection — if the provider has a `models`
///   object, only `models[route].upstream_model` is consulted and `model_map`
///   is ignored (the proxy's `if let Some(models)` branch); otherwise the
///   legacy `model_map[route]` is used.
///
/// Returns `None` when the route is not routed by that target at all
/// ("route unset" — the proxy would have no entry either).
pub(crate) fn resolve_route_upstream_model(
    cfg: &serde_json::Value,
    provider_id: &str,
    profile_id: Option<&str>,
    route: &str,
) -> Option<String> {
    if provider_id == "gateway_profile" {
        return resolve_route_for_gateway_profile(cfg, profile_id.unwrap_or(""), route)
            .map(|(_, m)| m);
    }

    let providers = cfg.get("providers").and_then(|p| p.as_object())?;
    let provider = providers.get(provider_id)?;

    if provider_id == "openrouter" {
        let profiles = provider.get("profiles").and_then(|p| p.as_array())?;
        if profiles.is_empty() {
            return None;
        }
        let active_id = cfg
            .get("active_openrouter_profile_id")
            .and_then(|v| v.as_str());
        let profile = match profile_id {
            Some(pid) => profiles
                .iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(pid)),
            None => profiles
                .iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == active_id)
                .or_else(|| profiles.first()),
        }?;
        return resolve_from_models(profile, route);
    }

    // Exclusive selection, matching the proxy: `models` present (as an object)
    // ⇒ models only; otherwise legacy model_map. `is_object` (not `is_some`)
    // treats `"models": null` as absent — the same as typed `Option` → `None`
    // taking the legacy model_map branch.
    if provider.get("models").map_or(false, serde_json::Value::is_object) {
        resolve_from_models(provider, route)
    } else {
        resolve_from_model_map(provider, route)
    }
}

pub(crate) const ELIGIBLE_GATEWAY_PROVIDERS: [&str; 4] = [
    "anthropic",
    "zai",
    "openrouter",
    "deepseek",
];

pub(crate) fn is_eligible_provider_model(provider: &str, model: &str) -> bool {
    match provider {
        "anthropic" => crate::ANTHROPIC_MODELS.contains(&model),
        "zai" => crate::ZAI_MODELS_PAY_AS_YOU_GO.contains(&model),
        "deepseek" => {
            matches!(
                model,
                "deepseek-v4-pro" | "deepseek-v4-flash" | "deepseek-chat" | "deepseek-reasoner"
            )
        }
        _ => false,
    }
}

/// Resolve the upstream model and provider for a route in a Gateway Profile.
///
/// Returns `Some((provider_id, upstream_model))` if the route is defined in the
/// Gateway Profile, references an eligible provider and model, and resolves to
/// a valid upstream model.
pub(crate) fn resolve_route_for_gateway_profile(
    cfg: &serde_json::Value,
    gateway_profile_id: &str,
    route: &str,
) -> Option<(String, String)> {
    let profiles = cfg.get("gateway_profiles").and_then(|p| p.as_array())?;
    let profile = profiles
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(gateway_profile_id))?;
    let routes = profile.get("routes").and_then(|r| r.as_object())?;
    let route_spec = routes.get(route)?;

    let provider = route_spec.get("provider").and_then(|p| p.as_str())?;
    if !ELIGIBLE_GATEWAY_PROVIDERS.contains(&provider) {
        return None;
    }
    if provider == "openrouter" {
        let profile_id = route_spec.get("profile_id").and_then(|p| p.as_str())?;
        let upstream = resolve_route_upstream_model(cfg, "openrouter", Some(profile_id), route)?;
        Some(("openrouter".to_string(), upstream))
    } else {
        let model = route_spec.get("model").and_then(|m| m.as_str())?;
        if !is_eligible_provider_model(provider, model) {
            return None;
        }
        Some((provider.to_string(), model.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_config() -> serde_json::Value {
        json!({
            "active_provider": "deepseek",
            "active_openrouter_profile_id": "prof-a",
            "providers": {
                "deepseek": {
                    "models": {
                        "claude-opus-5": { "upstream_model": "deepseek-v4-pro" },
                        "claude-sonnet-5": { "upstream_model": "deepseek-v4-flash" },
                        "claude-haiku-4-5": { "upstream_model": "deepseek-v4-flash" }
                    }
                },
                "openrouter": {
                    "profiles": [
                        {
                            "id": "prof-a",
                            "display_name": "OpenRouter: A",
                            "models": {
                                "claude-opus-5": { "upstream_model": "openai/gpt-5.6-sol" },
                                "claude-sonnet-5": { "upstream_model": "openai/gpt-5.6-terra" },
                                "claude-haiku-4-5": { "upstream_model": "openai/gpt-5.6-luna" }
                            }
                        },
                        {
                            "id": "prof-b",
                            "display_name": "OpenRouter: B",
                            "models": {
                                "claude-opus-5": { "upstream_model": "tencent/hy3" }
                            }
                        }
                    ]
                },
                "legacy": {
                    "model_map": {
                        "claude-opus-5": "some-legacy-model"
                    }
                }
            }
        })
    }

    #[test]
    fn direct_provider_models_explicit_used() {
        let cfg = base_config();
        assert_eq!(
            resolve_route_upstream_model(&cfg, "deepseek", None, "claude-opus-5"),
            Some("deepseek-v4-pro".to_string())
        );
        assert_eq!(
            resolve_route_upstream_model(&cfg, "deepseek", None, "claude-haiku-4-5"),
            Some("deepseek-v4-flash".to_string())
        );
    }

    #[test]
    fn direct_provider_model_map_fallback() {
        let cfg = base_config();
        assert_eq!(
            resolve_route_upstream_model(&cfg, "legacy", None, "claude-opus-5"),
            Some("some-legacy-model".to_string())
        );
    }

    #[test]
    fn openrouter_active_profile_by_id() {
        let cfg = base_config();
        assert_eq!(
            resolve_route_upstream_model(&cfg, "openrouter", None, "claude-opus-5"),
            Some("openai/gpt-5.6-sol".to_string())
        );
    }

    #[test]
    fn openrouter_explicit_profile_id() {
        let cfg = base_config();
        assert_eq!(
            resolve_route_upstream_model(&cfg, "openrouter", Some("prof-b"), "claude-opus-5"),
            Some("tencent/hy3".to_string())
        );
        assert_eq!(
            resolve_route_upstream_model(&cfg, "openrouter", Some("prof-b"), "claude-sonnet-5"),
            None
        );
    }

    #[test]
    fn openrouter_missing_profile_falls_back_to_first() {
        let mut cfg = base_config();
        cfg["active_openrouter_profile_id"] = json!("no-such-profile");
        assert_eq!(
            resolve_route_upstream_model(&cfg, "openrouter", None, "claude-opus-5"),
            Some("openai/gpt-5.6-sol".to_string())
        );
    }

    #[test]
    fn openrouter_no_profiles_returns_none() {
        let cfg = json!({
            "providers": { "openrouter": { "profiles": [] } }
        });
        assert_eq!(resolve_route_upstream_model(&cfg, "openrouter", None, "claude-opus-5"), None);
    }

    #[test]
    fn unknown_provider_or_route_returns_none() {
        let cfg = base_config();
        assert_eq!(resolve_route_upstream_model(&cfg, "no-such-provider", None, "claude-opus-5"), None);
        assert_eq!(resolve_route_upstream_model(&cfg, "deepseek", None, "claude-unknown-route"), None);
    }

    #[test]
    fn real_claude_route_names_resolve() {
        // The 3 canonical routes used by the resolver.
        let cfg = base_config();
        for route in CLAUDE_ROUTES {
            assert!(
                resolve_route_upstream_model(&cfg, "deepseek", None, route).is_some(),
                "route {} must resolve",
                route
            );
        }
    }

    #[test]
    fn direct_models_present_ignores_model_map() {
        // `models` present but lacking the route + `model_map` has it → None.
        // This is the exclusive selection (proxy behavior): once `models`
        // exists, `model_map` is never consulted per-key.
        let cfg = json!({
            "providers": {
                "mixed": {
                    "models": {
                        "claude-sonnet-5": { "upstream_model": "model-a" },
                        "claude-haiku-4-5": { "upstream_model": "model-b" }
                    },
                    "model_map": { "claude-opus-5": "model-c" }
                }
            }
        });
        assert_eq!(
            resolve_route_upstream_model(&cfg, "mixed", None, "claude-opus-5"),
            None
        );
        assert_eq!(
            resolve_route_upstream_model(&cfg, "mixed", None, "claude-sonnet-5"),
            Some("model-a".to_string())
        );
    }

    #[test]
    fn direct_empty_models_present_ignores_model_map() {
        // `models: {}` is "present" — the proxy enters the models branch and
        // iterates nothing, so model_map must NOT be used as a fallback.
        let cfg = json!({
            "providers": {
                "mixed": {
                    "models": {},
                    "model_map": { "claude-opus-5": "model-c" }
                }
            }
        });
        assert_eq!(
            resolve_route_upstream_model(&cfg, "mixed", None, "claude-opus-5"),
            None
        );
    }

    #[test]
    fn direct_models_null_uses_model_map() {
        // `"models": null` is treated as absent (typed `Option` → `None`),
        // so the legacy model_map branch is used.
        let cfg = json!({
            "providers": {
                "mixed": {
                    "models": null,
                    "model_map": { "claude-opus-5": "model-c" }
                }
            }
        });
        assert_eq!(
            resolve_route_upstream_model(&cfg, "mixed", None, "claude-opus-5"),
            Some("model-c".to_string())
        );
    }

    #[test]
    fn openrouter_profile_models_only_ignores_model_map() {
        // A profile without `models` must NOT fall back to its own `model_map`
        // (the proxy routes only from the active profile's `models`).
        let cfg = json!({
            "providers": {
                "openrouter": {
                    "profiles": [
                        {
                            "id": "prof-x",
                            "display_name": "X",
                            "model_map": { "claude-opus-5": "some-model" }
                        }
                    ]
                }
            }
        });
        assert_eq!(
            resolve_route_upstream_model(&cfg, "openrouter", None, "claude-opus-5"),
            None
        );
    }

    #[test]
    fn resolve_from_models_and_model_map_extract_only() {
        let scope = json!({
            "models": { "claude-opus-5": { "upstream_model": "model-a" } },
            "model_map": { "claude-opus-5": "model-b", "claude-sonnet-5": "model-c" }
        });
        // resolve_from_models never consults model_map.
        assert_eq!(resolve_from_models(&scope, "claude-opus-5"), Some("model-a".to_string()));
        assert_eq!(resolve_from_models(&scope, "claude-sonnet-5"), None);
        // resolve_from_model_map never consults models.
        assert_eq!(resolve_from_model_map(&scope, "claude-opus-5"), Some("model-b".to_string()));
        assert_eq!(resolve_from_model_map(&scope, "claude-sonnet-5"), Some("model-c".to_string()));
        // Missing maps / routes → None.
        assert_eq!(resolve_from_models(&scope, "claude-unknown"), None);
        let no_maps = json!({});
        assert_eq!(resolve_from_models(&no_maps, "claude-opus-5"), None);
        assert_eq!(resolve_from_model_map(&no_maps, "claude-opus-5"), None);
    }

    #[test]
    fn claude_55_routes_resolve_only_where_mapped() {
        let cfg = json!({
            "providers": {
                "anthropic-profile": {
                    "models": {
                        "claude-opus-5-5": { "upstream_model": "anthropic/claude-opus-5.5" },
                        "claude-sonnet-5-5": { "upstream_model": "anthropic/claude-sonnet-5.5" },
                        "claude-haiku-5-5": { "upstream_model": "anthropic/claude-haiku-5.5" }
                    }
                },
                "deepseek": {
                    "models": { "claude-opus-5": { "upstream_model": "deepseek-flash" } }
                }
            }
        });

        for route in CLAUDE_55_ROUTES {
            assert!(
                resolve_route_upstream_model(&cfg, "anthropic-profile", None, route).is_some(),
                "{route} must resolve on the mapped target"
            );
            assert_eq!(
                resolve_route_upstream_model(&cfg, "deepseek", None, route),
                None,
                "{route} must stay unresolved where it has no mapping"
            );
        }
        // The base routes are unaffected by the 5.5 additions.
        assert_eq!(
            resolve_route_upstream_model(&cfg, "deepseek", None, "claude-opus-5"),
            Some("deepseek-flash".to_string())
        );
    }

    #[test]
    fn gateway_profile_route_resolution() {
        let cfg = json!({
            "active_gateway_profile_id": "gw-1",
            "gateway_profiles": [
                {
                    "id": "gw-1",
                    "display_name": "Multi-Provider Mix",
                    "routes": {
                        "claude-opus-5": {
                            "provider": "anthropic",
                            "model": "claude-opus-5-5"
                        },
                        "claude-sonnet-5": {
                            "provider": "zai",
                            "model": "glm-5.3"
                        },
                        "claude-haiku-4-5": {
                            "provider": "openrouter",
                            "profile_id": "or-prof-1"
                        }
                    }
                }
            ],
            "providers": {
                "openrouter": {
                    "profiles": [
                        {
                            "id": "or-prof-1",
                            "display_name": "OpenRouter: Laguna",
                            "models": {
                                "claude-haiku-4-5": {
                                    "upstream_model": "poolside/laguna-xs-2.1"
                                }
                            }
                        }
                    ]
                }
            }
        });

        // 1. Direct model routes
        assert_eq!(
            resolve_route_for_gateway_profile(&cfg, "gw-1", "claude-opus-5"),
            Some(("anthropic".to_string(), "claude-opus-5-5".to_string()))
        );
        assert_eq!(
            resolve_route_for_gateway_profile(&cfg, "gw-1", "claude-sonnet-5"),
            Some(("zai".to_string(), "glm-5.3".to_string()))
        );

        // 2. OpenRouter profile reference
        assert_eq!(
            resolve_route_for_gateway_profile(&cfg, "gw-1", "claude-haiku-4-5"),
            Some(("openrouter".to_string(), "poolside/laguna-xs-2.1".to_string()))
        );

        // 3. Via resolve_route_upstream_model with provider_id="gateway_profile"
        assert_eq!(
            resolve_route_upstream_model(&cfg, "gateway_profile", Some("gw-1"), "claude-opus-5"),
            Some("claude-opus-5-5".to_string())
        );
        assert_eq!(
            resolve_route_upstream_model(&cfg, "gateway_profile", Some("gw-1"), "claude-sonnet-5"),
            Some("glm-5.3".to_string())
        );
        assert_eq!(
            resolve_route_upstream_model(&cfg, "gateway_profile", Some("gw-1"), "claude-haiku-4-5"),
            Some("poolside/laguna-xs-2.1".to_string())
        );

        // 4. Missing route or unknown profile returns None
        assert_eq!(resolve_route_for_gateway_profile(&cfg, "gw-1", "claude-unknown"), None);
        assert_eq!(resolve_route_for_gateway_profile(&cfg, "gw-unknown", "claude-opus-5"), None);
    }
}
