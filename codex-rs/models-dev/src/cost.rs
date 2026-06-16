use crate::schema::ModelsDevModelCost;

/// Token counts used to estimate API cost from models.dev pricing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenBillableUsage {
    pub non_cached_input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
}

impl TokenBillableUsage {
    pub fn is_zero(&self) -> bool {
        self.non_cached_input_tokens <= 0
            && self.cached_input_tokens <= 0
            && self.output_tokens <= 0
            && self.reasoning_output_tokens <= 0
    }

    pub fn subtract(self, other: Self) -> Self {
        Self {
            non_cached_input_tokens: self
                .non_cached_input_tokens
                .saturating_sub(other.non_cached_input_tokens),
            cached_input_tokens: self
                .cached_input_tokens
                .saturating_sub(other.cached_input_tokens),
            output_tokens: self.output_tokens.saturating_sub(other.output_tokens),
            reasoning_output_tokens: self
                .reasoning_output_tokens
                .saturating_sub(other.reasoning_output_tokens),
        }
    }
}

/// Estimates USD cost using models.dev per-million-token rates.
pub fn estimate_cost_usd(usage: TokenBillableUsage, cost: &ModelsDevModelCost) -> f64 {
    let per_million = 1_000_000.0;
    (usage.non_cached_input_tokens.max(0) as f64 * cost.input
        + usage.cached_input_tokens.max(0) as f64 * cost.cache_read
        + usage.output_tokens.max(0) as f64 * cost.output
        + usage.reasoning_output_tokens.max(0) as f64 * cost.output)
        / per_million
}

/// Formats a USD cost estimate for statusline display.
pub fn format_cost_estimate_usd(cost_usd: f64) -> String {
    if cost_usd <= 0.0 {
        return "~$0 est.".to_string();
    }
    if cost_usd < 0.01 {
        return format!("~${cost_usd:.4} est.");
    }
    if cost_usd < 1.0 {
        return format!("~${cost_usd:.2} est.");
    }
    format!("~${cost_usd:.2} est.")
}

/// Formats a per-turn USD cost estimate for statusline display.
pub fn format_turn_cost_estimate_usd(cost_usd: f64) -> String {
    if cost_usd <= 0.0 {
        return "~$0 turn".to_string();
    }
    if cost_usd < 0.01 {
        return format!("~${cost_usd:.4} turn");
    }
    if cost_usd < 1.0 {
        return format!("~${cost_usd:.2} turn");
    }
    format!("~${cost_usd:.2} turn")
}
