use crate::cost::TokenBillableUsage;
use crate::cost::estimate_cost_usd;
use crate::cost::format_cost_estimate_usd;
use crate::cost::format_turn_cost_estimate_usd;
use crate::schema::ModelsDevModelCost;
use pretty_assertions::assert_eq;

#[test]
fn estimate_cost_usd_applies_per_million_rates() {
    let cost = ModelsDevModelCost {
        input: 2.0,
        output: 8.0,
        cache_read: 0.5,
        cache_write: 0.0,
    };
    let usage = TokenBillableUsage {
        non_cached_input_tokens: 1_000_000,
        cached_input_tokens: 0,
        output_tokens: 500_000,
        reasoning_output_tokens: 0,
    };

    assert_eq!(estimate_cost_usd(usage, &cost), 6.0);
}

#[test]
fn estimate_cost_usd_counts_reasoning_as_output() {
    let cost = ModelsDevModelCost {
        input: 0.0,
        output: 10.0,
        cache_read: 0.0,
        cache_write: 0.0,
    };
    let usage = TokenBillableUsage {
        non_cached_input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 100_000,
    };

    assert_eq!(estimate_cost_usd(usage, &cost), 1.0);
}

#[test]
fn format_cost_estimate_usd_uses_compact_precision() {
    assert_eq!(format_cost_estimate_usd(0.0), "~$0 est.");
    assert_eq!(format_cost_estimate_usd(0.0042), "~$0.0042 est.");
    assert_eq!(format_cost_estimate_usd(0.42), "~$0.42 est.");
    assert_eq!(format_cost_estimate_usd(12.34), "~$12.34 est.");
}

#[test]
fn format_turn_cost_estimate_usd_labels_turn() {
    assert_eq!(format_turn_cost_estimate_usd(0.03), "~$0.03 turn");
}
