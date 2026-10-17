use super::adapters::NormalizedUsage;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 1 USD = 1,000,000,000 nano-USD.
pub const NANOS_PER_USD: u64 = 1_000_000_000;

/// Maximum safe integer representable without precision loss in standard IEEE 754 JavaScript numbers (2^53 - 1).
/// Corresponds to $9,007,199.254740991 USD.
pub const JS_MAX_SAFE_NANOS: u64 = 9_007_199_254_740_991;

/// Serializes `amount_nanos` as a 10-decimal string.
/// Guarantees that values up to `u64::MAX` are transmitted across JSON/IPC to JavaScript
/// without IEEE 754 precision loss beyond `JS_MAX_SAFE_NANOS` (2^53 - 1).
pub fn serialize_amount_nanos<S>(val: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&val.to_string())
}

/// Deserializes `amount_nanos` from either a JSON integer or a JSON decimal string.
/// This allows transporting large u64 values safely across boundaries without float truncation.
pub fn deserialize_amount_nanos<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct AmountNanosVisitor;

    impl<'de> serde::de::Visitor<'de> for AmountNanosVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a non-negative integer or decimal integer string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_u128<E>(self, value: u128) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            u64::try_from(value)
                .map_err(|_| serde::de::Error::custom("amount_nanos exceeds u64::MAX"))
        }

        fn visit_i64<E>(self, value: i64) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            if value < 0 {
                Err(serde::de::Error::custom("amount_nanos cannot be negative"))
            } else {
                Ok(value as u64)
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            value.parse::<u64>().map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_any(AmountNanosVisitor)
}

/// Immutable financial cost estimate in fixed-point integer nano-units.
/// Floating-point types (f32/f64) are strictly prohibited for arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimatedCost {
    /// ISO 4217 currency code (e.g. "USD").
    pub currency: String,
    /// Cost amount represented in nano-units (1/1,000,000,000 of the currency unit).
    /// Serialized as a 10-decimal string to guarantee exact transport across IPC/JSON to JavaScript without IEEE 754 precision loss.
    #[serde(
        serialize_with = "serialize_amount_nanos",
        deserialize_with = "deserialize_amount_nanos"
    )]
    pub amount_nanos: u64,
}

impl EstimatedCost {
    pub fn new_usd(amount_nanos: u64) -> Self {
        Self {
            currency: "USD".to_string(),
            amount_nanos,
        }
    }

    /// Checked addition of two same-currency costs.
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        if self.currency != other.currency {
            return None;
        }
        let amount = self.amount_nanos.checked_add(other.amount_nanos)?;
        Some(Self {
            currency: self.currency.clone(),
            amount_nanos: amount,
        })
    }

    /// Formats the cost into human-readable currency representation (e.g. "$0.0140", "$0.05", "< $0.0001").
    /// Preserves non-zero amounts so positive estimates are never displayed as zero.
    pub fn format_display(&self) -> String {
        if self.amount_nanos == 0 {
            return if self.currency == "USD" {
                "$0.00".to_string()
            } else {
                format!("0.00 {}", self.currency)
            };
        }

        // Sub-cent floor: any positive amount less than 0.0001 currency unit (100,000 nanos)
        // must never be displayed as $0.00 or $0.0000.
        if self.amount_nanos < 100_000 {
            return if self.currency == "USD" {
                "< $0.0001".to_string()
            } else {
                format!("< 0.0001 {}", self.currency)
            };
        }

        let whole = self.amount_nanos / NANOS_PER_USD;
        let rem = self.amount_nanos % NANOS_PER_USD;

        // Exact cents (no sub-cent remainder at all)
        if rem % 10_000_000 == 0 {
            let cents = rem / 10_000_000;
            return if self.currency == "USD" {
                format!("${}.{:02}", whole, cents)
            } else {
                format!("{}.{:02} {}", whole, cents, self.currency)
            };
        }

        // Round half-up to 4 decimal places (each 0.0001 unit is 100,000 nanos; half-unit is 50,000)
        let rounded_rem = rem + 50_000;
        let (final_whole, frac) = if rounded_rem >= NANOS_PER_USD {
            (
                whole.saturating_add(1),
                (rounded_rem - NANOS_PER_USD) / 100_000,
            )
        } else {
            (whole, rounded_rem / 100_000)
        };

        if self.currency == "USD" {
            format!("${}.{:04}", final_whole, frac)
        } else {
            format!("{}.{:04} {}", final_whole, frac, self.currency)
        }
    }
}

/// Status of the run-level aggregated cost estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunCostStatus {
    /// All dispatches with reported usage were successfully priced.
    Complete,
    /// Some dispatches were priced, but one or more could not be priced (unsupported/unknown schedule).
    Partial,
    /// No dispatches could be priced.
    Unavailable,
}

/// Default method name for bundled rate calculations.
pub const BUNDLED_EXACT_RATE_METHOD: &str = "bundled_exact_rate_v1";

/// Run-level aggregated cost summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCostSummary {
    pub currency: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_cost: Option<EstimatedCost>,
    pub status: RunCostStatus,
    pub priced_dispatches: u64,
    pub unpriced_dispatches: u64,
    /// Rate schedule identifiers contributing to this estimate (provenance).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schedules: Vec<String>,
    /// Provenance calculation method identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

/// Immutable pricing schedule for a proven provider and model route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateSchedule {
    pub schedule_id: &'static str,
    pub provider_id: &'static str,
    pub model_id: &'static str,
    pub currency: &'static str,
    pub input_rate_nanos_per_token: u64,
    pub output_rate_nanos_per_token: u64,
    pub source_url: &'static str,
    pub effective_date: &'static str,
    pub verification_date: &'static str,
}

/// Official rates for Anthropic Direct (Verified: 2026-10-17).
/// - Claude Opus 5.5: Input $4.00/1M tokens = 4,000 nUSD/token; Output $20.00/1M = 20,000 nUSD/token.
pub static ANTHROPIC_OPUS_55_UNCACHED: RateSchedule = RateSchedule {
    schedule_id: "anthropic:claude-opus-5-5:2026-10:uncached",
    provider_id: "anthropic",
    model_id: "claude-opus-5-5",
    currency: "USD",
    input_rate_nanos_per_token: 4_000,
    output_rate_nanos_per_token: 20_000,
    source_url: "https://www.anthropic.com/pricing",
    effective_date: "2026-10",
    verification_date: "2026-10-17",
};

/// Official rates for Anthropic Direct (Verified: 2026-10-17).
/// - Claude Sonnet 5.5: Input $2.00/1M tokens = 2,000 nUSD/token; Output $10.00/1M = 10,000 nUSD/token.
pub static ANTHROPIC_SONNET_55_UNCACHED: RateSchedule = RateSchedule {
    schedule_id: "anthropic:claude-sonnet-5-5:2026-10:uncached",
    provider_id: "anthropic",
    model_id: "claude-sonnet-5-5",
    currency: "USD",
    input_rate_nanos_per_token: 2_000,
    output_rate_nanos_per_token: 10_000,
    source_url: "https://www.anthropic.com/pricing",
    effective_date: "2026-10",
    verification_date: "2026-10-17",
};

/// Bundled immutable schedule registry.
pub static BUNDLED_SCHEDULES: &[RateSchedule] =
    &[ANTHROPIC_OPUS_55_UNCACHED, ANTHROPIC_SONNET_55_UNCACHED];

/// Exact matching of provider and model to an immutable pricing schedule.
/// No fallback to nearby models or alternate providers is permitted.
pub fn lookup_rate_schedule(provider_id: &str, model: &str) -> Option<&'static RateSchedule> {
    BUNDLED_SCHEDULES
        .iter()
        .find(|s| s.provider_id == provider_id && s.model_id == model)
}

/// Looks up a bundled rate schedule by its exact schedule ID and verifies consistency
/// with the given provider, model, and currency.
/// Returns Some(&'static RateSchedule) only if all attributes match an entry in the
/// immutable bundled registry exactly.
pub fn lookup_and_verify_schedule(
    schedule_id: &str,
    provider_id: Option<&str>,
    model: Option<&str>,
    currency: &str,
) -> Option<&'static RateSchedule> {
    if schedule_id.trim().is_empty() {
        return None;
    }
    let prov = provider_id?;
    let mod_id = model?;
    BUNDLED_SCHEDULES.iter().find(|s| {
        s.schedule_id == schedule_id
            && s.provider_id == prov
            && s.model_id == mod_id
            && s.currency == currency
    })
}

/// Evaluates whether an adapter dispatch qualifies for cost estimation and computes its estimate.
///
/// Fail-closed rules per V0.25.4-r2 §5.3 / §5.8:
/// 1. Only proven uncached Anthropic Direct Opus 5.5 and Sonnet 5.5 routes are supported.
/// 2. `input_tokens` and `output_tokens` must both be present.
/// 3. Both `cache_read_tokens` and `cache_write_tokens` MUST be explicitly `Some(0)`.
///    If either is `None` (omitted/malformed) or `> 0`, the dispatch cannot be priced.
/// 4. All other models (Haiku, DeepSeek, OpenRouter, etc.) and non-provider adapters return None.
pub fn estimate_dispatch_cost(
    provider_id: Option<&str>,
    model: Option<&str>,
    usage: Option<&NormalizedUsage>,
) -> Option<(String, EstimatedCost)> {
    let provider = provider_id?;
    let model_id = model?;
    let u = usage?;

    // Look up exact rate schedule
    let schedule = lookup_rate_schedule(provider, model_id)?;

    let input_tokens = u.input_tokens?;
    let output_tokens = u.output_tokens?;

    // Fail-closed cache check: MUST be explicitly Some(0)
    match (u.cache_read_tokens, u.cache_write_tokens) {
        (Some(0), Some(0)) => {}
        _ => return None,
    }

    // Checked arithmetic in nano-units
    let input_cost = input_tokens.checked_mul(schedule.input_rate_nanos_per_token)?;
    let output_cost = output_tokens.checked_mul(schedule.output_rate_nanos_per_token)?;
    let total_nanos = input_cost.checked_add(output_cost)?;

    Some((
        schedule.schedule_id.to_string(),
        EstimatedCost {
            currency: schedule.currency.to_string(),
            amount_nanos: total_nanos,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_rate_schedule_exact_matches() {
        let opus = lookup_rate_schedule("anthropic", "claude-opus-5-5").unwrap();
        assert_eq!(
            opus.schedule_id,
            "anthropic:claude-opus-5-5:2026-10:uncached"
        );
        assert_eq!(opus.input_rate_nanos_per_token, 4_000);
        assert_eq!(opus.output_rate_nanos_per_token, 20_000);

        let sonnet = lookup_rate_schedule("anthropic", "claude-sonnet-5-5").unwrap();
        assert_eq!(
            sonnet.schedule_id,
            "anthropic:claude-sonnet-5-5:2026-10:uncached"
        );
        assert_eq!(sonnet.input_rate_nanos_per_token, 2_000);
        assert_eq!(sonnet.output_rate_nanos_per_token, 10_000);
    }

    #[test]
    fn test_lookup_rate_schedule_no_fallbacks() {
        // Haiku is unsupported
        assert!(lookup_rate_schedule("anthropic", "claude-haiku-5-5").is_none());
        assert!(lookup_rate_schedule("anthropic", "claude-haiku-4-5").is_none());
        // DeepSeek is unsupported
        assert!(lookup_rate_schedule("deepseek", "deepseek-flash").is_none());
        assert!(lookup_rate_schedule("deepseek", "deepseek-v4-pro").is_none());
        // OpenRouter is unsupported
        assert!(lookup_rate_schedule("openrouter", "anthropic/claude-opus-5.5").is_none());
        // Unknown provider/model
        assert!(lookup_rate_schedule("unknown", "custom-model").is_none());
        assert!(lookup_rate_schedule("anthropic", "claude-3-7-sonnet").is_none());
    }

    #[test]
    fn test_lookup_and_verify_schedule() {
        // Exact matching for Opus 5.5
        let opus = lookup_and_verify_schedule(
            "anthropic:claude-opus-5-5:2026-10:uncached",
            Some("anthropic"),
            Some("claude-opus-5-5"),
            "USD",
        )
        .expect("exact Opus match");
        assert_eq!(
            opus.schedule_id,
            "anthropic:claude-opus-5-5:2026-10:uncached"
        );

        // Exact matching for Sonnet 5.5
        let sonnet = lookup_and_verify_schedule(
            "anthropic:claude-sonnet-5-5:2026-10:uncached",
            Some("anthropic"),
            Some("claude-sonnet-5-5"),
            "USD",
        )
        .expect("exact Sonnet match");
        assert_eq!(
            sonnet.schedule_id,
            "anthropic:claude-sonnet-5-5:2026-10:uncached"
        );

        // Model mismatch -> None
        assert!(lookup_and_verify_schedule(
            "anthropic:claude-opus-5-5:2026-10:uncached",
            Some("anthropic"),
            Some("claude-sonnet-5-5"),
            "USD",
        )
        .is_none());

        // Provider mismatch -> None
        assert!(lookup_and_verify_schedule(
            "anthropic:claude-opus-5-5:2026-10:uncached",
            Some("deepseek"),
            Some("claude-opus-5-5"),
            "USD",
        )
        .is_none());

        // Currency mismatch -> None
        assert!(lookup_and_verify_schedule(
            "anthropic:claude-opus-5-5:2026-10:uncached",
            Some("anthropic"),
            Some("claude-opus-5-5"),
            "EUR",
        )
        .is_none());

        // Unknown schedule ID -> None
        assert!(lookup_and_verify_schedule(
            "custom:model:2026-10",
            Some("anthropic"),
            Some("claude-opus-5-5"),
            "USD",
        )
        .is_none());

        // Empty schedule ID -> None
        assert!(
            lookup_and_verify_schedule("", Some("anthropic"), Some("claude-opus-5-5"), "USD",)
                .is_none()
        );

        // Missing provider or model -> None
        assert!(lookup_and_verify_schedule(
            "anthropic:claude-opus-5-5:2026-10:uncached",
            None,
            Some("claude-opus-5-5"),
            "USD",
        )
        .is_none());
        assert!(lookup_and_verify_schedule(
            "anthropic:claude-opus-5-5:2026-10:uncached",
            Some("anthropic"),
            None,
            "USD",
        )
        .is_none());
    }

    #[test]
    fn test_estimate_dispatch_cost_opus_uncached_success() {
        let usage = NormalizedUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            raw_total_tokens: Some(1_500),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        };

        let (sched_id, cost) =
            estimate_dispatch_cost(Some("anthropic"), Some("claude-opus-5-5"), Some(&usage))
                .expect("should estimate cost");

        assert_eq!(sched_id, "anthropic:claude-opus-5-5:2026-10:uncached");
        assert_eq!(cost.currency, "USD");
        // 1,000 * 4,000 + 500 * 20,000 = 4,000,000 + 10,000,000 = 14,000,000 nanos ($0.014)
        assert_eq!(cost.amount_nanos, 14_000_000);
        assert_eq!(cost.format_display(), "$0.0140");
    }

    #[test]
    fn test_estimate_dispatch_cost_sonnet_uncached_success() {
        let usage = NormalizedUsage {
            input_tokens: Some(2_000),
            output_tokens: Some(100),
            raw_total_tokens: Some(2_100),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        };

        let (sched_id, cost) =
            estimate_dispatch_cost(Some("anthropic"), Some("claude-sonnet-5-5"), Some(&usage))
                .expect("should estimate cost");

        assert_eq!(sched_id, "anthropic:claude-sonnet-5-5:2026-10:uncached");
        assert_eq!(cost.currency, "USD");
        // 2,000 * 2,000 + 100 * 10,000 = 4,000,000 + 1,000,000 = 5,000,000 nanos ($0.005)
        assert_eq!(cost.amount_nanos, 5_000_000);
        assert_eq!(cost.format_display(), "$0.0050");
    }

    #[test]
    fn test_estimate_dispatch_cost_rejects_missing_or_nonzero_cache_tokens() {
        // 1. None in cache_read_tokens -> Rejected
        let missing_cache_read = NormalizedUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            raw_total_tokens: Some(1_500),
            cache_read_tokens: None,
            cache_write_tokens: Some(0),
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&missing_cache_read)
        )
        .is_none());

        // 2. None in cache_write_tokens -> Rejected
        let missing_cache_write = NormalizedUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            raw_total_tokens: Some(1_500),
            cache_read_tokens: Some(0),
            cache_write_tokens: None,
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&missing_cache_write)
        )
        .is_none());

        // 3. Both None -> Rejected
        let both_missing = NormalizedUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            raw_total_tokens: Some(1_500),
            cache_read_tokens: None,
            cache_write_tokens: None,
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&both_missing)
        )
        .is_none());

        // 4. Non-zero cache read -> Rejected
        let nonzero_read = NormalizedUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            raw_total_tokens: Some(1_500),
            cache_read_tokens: Some(10),
            cache_write_tokens: Some(0),
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&nonzero_read)
        )
        .is_none());

        // 5. Non-zero cache write -> Rejected
        let nonzero_write = NormalizedUsage {
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            raw_total_tokens: Some(1_500),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(25),
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&nonzero_write)
        )
        .is_none());
    }

    #[test]
    fn test_estimate_dispatch_cost_rejects_missing_input_or_output() {
        let missing_inp = NormalizedUsage {
            input_tokens: None,
            output_tokens: Some(500),
            raw_total_tokens: None,
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&missing_inp)
        )
        .is_none());

        let missing_out = NormalizedUsage {
            input_tokens: Some(100),
            output_tokens: None,
            raw_total_tokens: None,
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&missing_out)
        )
        .is_none());
    }

    #[test]
    fn test_cost_checked_add_same_and_different_currency() {
        let c1 = EstimatedCost::new_usd(1_000_000);
        let c2 = EstimatedCost::new_usd(2_500_000);
        let sum = c1.checked_add(&c2).unwrap();
        assert_eq!(sum.currency, "USD");
        assert_eq!(sum.amount_nanos, 3_500_000);

        let c_eur = EstimatedCost {
            currency: "EUR".to_string(),
            amount_nanos: 1_000_000,
        };
        assert!(c1.checked_add(&c_eur).is_none());
    }

    #[test]
    fn test_cost_arithmetic_overflow_safety() {
        let max_cost = EstimatedCost::new_usd(u64::MAX);
        let one = EstimatedCost::new_usd(1);
        assert!(max_cost.checked_add(&one).is_none());

        let huge_usage = NormalizedUsage {
            input_tokens: Some(u64::MAX),
            output_tokens: Some(10),
            raw_total_tokens: None,
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        };
        assert!(estimate_dispatch_cost(
            Some("anthropic"),
            Some("claude-opus-5-5"),
            Some(&huge_usage)
        )
        .is_none());
    }

    #[test]
    fn test_cost_format_display_boundaries_and_precision() {
        // 1. Zero amount
        assert_eq!(EstimatedCost::new_usd(0).format_display(), "$0.00");
        assert_eq!(
            EstimatedCost {
                currency: "EUR".to_string(),
                amount_nanos: 0,
            }
            .format_display(),
            "0.00 EUR"
        );

        // 2. Small positive amounts strictly below 0.0001 (100,000 nanos) must never truncate to $0.00
        assert_eq!(EstimatedCost::new_usd(1).format_display(), "< $0.0001");
        assert_eq!(EstimatedCost::new_usd(99_999).format_display(), "< $0.0001");
        assert_eq!(
            EstimatedCost {
                currency: "EUR".to_string(),
                amount_nanos: 50,
            }
            .format_display(),
            "< 0.0001 EUR"
        );

        // 3. Exact threshold at 100,000 nanos ($0.0001)
        assert_eq!(EstimatedCost::new_usd(100_000).format_display(), "$0.0001");

        // 4. Exact cents formatting (2 decimal places)
        assert_eq!(EstimatedCost::new_usd(50_000_000).format_display(), "$0.05");
        assert_eq!(
            EstimatedCost::new_usd(1_000_000_000).format_display(),
            "$1.00"
        );
        assert_eq!(
            EstimatedCost::new_usd(2_500_000_000).format_display(),
            "$2.50"
        );
        assert_eq!(
            EstimatedCost {
                currency: "EUR".to_string(),
                amount_nanos: 50_000_000,
            }
            .format_display(),
            "0.05 EUR"
        );

        // 5. Sub-cent rounding half-up at 4 decimal places
        // $0.123400000 -> $0.1234
        assert_eq!(
            EstimatedCost::new_usd(123_400_000).format_display(),
            "$0.1234"
        );
        // $0.123449999 -> rounds down to $0.1234
        assert_eq!(
            EstimatedCost::new_usd(123_449_999).format_display(),
            "$0.1234"
        );
        // $0.123450000 -> rounds half-up to $0.1235
        assert_eq!(
            EstimatedCost::new_usd(123_450_000).format_display(),
            "$0.1235"
        );

        // 6. Number.MAX_SAFE_INTEGER (9_007_199_254_740_991 nanos = $9,007,199.254740991)
        assert_eq!(
            EstimatedCost::new_usd(JS_MAX_SAFE_NANOS).format_display(),
            "$9007199.2547"
        );

        // 7. Maximum supported value (u64::MAX = 18_446_744_073_709_551_615 nanos)
        assert_eq!(
            EstimatedCost::new_usd(u64::MAX).format_display(),
            "$18446744073.7096"
        );
    }

    #[test]
    fn test_estimated_cost_serde_roundtrip_numbers_and_strings() {
        // Deserializing from JSON number
        let from_num: EstimatedCost =
            serde_json::from_str(r#"{"currency":"USD","amountNanos":50000000}"#).unwrap();
        assert_eq!(from_num.currency, "USD");
        assert_eq!(from_num.amount_nanos, 50_000_000);

        // Deserializing from JSON string
        let from_str: EstimatedCost =
            serde_json::from_str(r#"{"currency":"USD","amountNanos":"50000000"}"#).unwrap();
        assert_eq!(from_str.currency, "USD");
        assert_eq!(from_str.amount_nanos, 50_000_000);

        // Deserializing JS_MAX_SAFE_NANOS from string
        let from_safe: EstimatedCost = serde_json::from_str(&format!(
            r#"{{"currency":"USD","amountNanos":"{}"}}"#,
            JS_MAX_SAFE_NANOS
        ))
        .unwrap();
        assert_eq!(from_safe.amount_nanos, JS_MAX_SAFE_NANOS);

        // Deserializing u64::MAX from string
        let from_max: EstimatedCost = serde_json::from_str(&format!(
            r#"{{"currency":"USD","amountNanos":"{}"}}"#,
            u64::MAX
        ))
        .unwrap();
        assert_eq!(from_max.amount_nanos, u64::MAX);

        // Serializing produces decimal string to prevent JS number float64 precision loss
        let cost = EstimatedCost::new_usd(50_000_000);
        let serialized = serde_json::to_string(&cost).unwrap();
        assert_eq!(serialized, r#"{"currency":"USD","amountNanos":"50000000"}"#);

        let max_cost = EstimatedCost::new_usd(u64::MAX);
        let serialized_max = serde_json::to_string(&max_cost).unwrap();
        assert_eq!(
            serialized_max,
            r#"{"currency":"USD","amountNanos":"18446744073709551615"}"#
        );

        let roundtrip_max: EstimatedCost = serde_json::from_str(&serialized_max).unwrap();
        assert_eq!(roundtrip_max.amount_nanos, u64::MAX);

        // Negative numbers or invalid strings are rejected
        assert!(
            serde_json::from_str::<EstimatedCost>(r#"{"currency":"USD","amountNanos":-1}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<EstimatedCost>(r#"{"currency":"USD","amountNanos":"abc"}"#)
                .is_err()
        );
    }

    #[test]
    fn test_run_cost_summary_provenance_serde() {
        let summary = RunCostSummary {
            currency: "USD".to_string(),
            total_cost: Some(EstimatedCost::new_usd(50_000_000)),
            status: RunCostStatus::Complete,
            priced_dispatches: 1,
            unpriced_dispatches: 0,
            schedules: vec!["anthropic:claude-opus-5-5:2026-10:uncached".to_string()],
            method: Some(BUNDLED_EXACT_RATE_METHOD.to_string()),
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains(r#""schedules":["anthropic:claude-opus-5-5:2026-10:uncached"]"#));
        assert!(json.contains(r#""method":"bundled_exact_rate_v1""#));

        let deserialized: RunCostSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, summary);

        // Deserializing legacy JSON without schedules or method defaults to empty / None
        let legacy_json =
            r#"{"currency":"USD","status":"complete","pricedDispatches":1,"unpricedDispatches":0}"#;
        let legacy: RunCostSummary = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy.schedules, Vec::<String>::new());
        assert_eq!(legacy.method, None);
    }
}
