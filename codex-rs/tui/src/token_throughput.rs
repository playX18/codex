//! Token throughput tracking and display helpers for the TUI statusline.

use std::time::Instant;

use crate::token_usage::TokenUsage;

const MIN_STREAMING_ELAPSED_SEC: f64 = 0.5;
const MIN_COMPLETED_ELAPSED_SEC: f64 = 0.001;

/// Estimates token count from streamed text using a simple chars/4 heuristic.
pub(crate) fn estimate_tokens_from_text(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    let chars = text.chars().count() as u64;
    chars.div_ceil(4)
}

pub(crate) fn streaming_tokens_per_second(
    estimated_tokens: u64,
    started_at: Instant,
    now: Instant,
) -> Option<f64> {
    if estimated_tokens == 0 {
        return None;
    }
    let elapsed_secs = now.saturating_duration_since(started_at).as_secs_f64();
    if elapsed_secs < MIN_STREAMING_ELAPSED_SEC {
        return None;
    }
    Some(estimated_tokens as f64 / elapsed_secs)
}

pub(crate) fn completed_tokens_per_second(
    output_tokens: i64,
    reasoning_tokens: i64,
    started_at: Instant,
    completed_at: Instant,
) -> Option<f64> {
    let tokens = output_tokens.max(0) as u64 + reasoning_tokens.max(0) as u64;
    if tokens == 0 {
        return None;
    }
    let elapsed_secs = completed_at
        .saturating_duration_since(started_at)
        .as_secs_f64();
    if elapsed_secs < MIN_COMPLETED_ELAPSED_SEC {
        return None;
    }
    Some(tokens as f64 / elapsed_secs)
}

pub(crate) fn format_tokens_per_second(tokens_per_second: Option<f64>) -> Option<String> {
    let tokens_per_second = tokens_per_second?;
    if tokens_per_second < 1.0 {
        return Some("<1 t/s".to_string());
    }
    Some(format!("{} t/s", tokens_per_second.round() as u64))
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TokenThroughputState {
    stream_started_at: Option<Instant>,
    last_completed_tokens_per_second: Option<u64>,
    turn_usage_at_start: Option<TokenUsage>,
    last_turn_cost_usd: Option<f64>,
}

impl TokenThroughputState {
    pub(crate) fn on_turn_started(&mut self, usage_at_start: Option<TokenUsage>) {
        self.stream_started_at = None;
        self.turn_usage_at_start = usage_at_start;
    }

    pub(crate) fn note_stream_activity(&mut self, now: Instant) {
        if self.stream_started_at.is_none() {
            self.stream_started_at = Some(now);
        }
    }

    pub(crate) fn on_turn_completed(
        &mut self,
        started_at: Instant,
        completed_at: Instant,
        last_token_usage: &TokenUsage,
        turn_cost_usd: Option<f64>,
    ) {
        self.stream_started_at = None;
        self.last_turn_cost_usd = turn_cost_usd;
        self.last_completed_tokens_per_second = completed_tokens_per_second(
            last_token_usage.output_tokens,
            last_token_usage.reasoning_output_tokens,
            started_at,
            completed_at,
        )
        .map(|tokens_per_second| tokens_per_second.round() as u64);
        self.turn_usage_at_start = None;
    }

    pub(crate) fn turn_usage_at_start(&self) -> Option<&TokenUsage> {
        self.turn_usage_at_start.as_ref()
    }

    pub(crate) fn last_turn_cost_usd(&self) -> Option<f64> {
        self.last_turn_cost_usd
    }

    pub(crate) fn display_value(
        &self,
        now: Instant,
        agent_stream_text: &str,
        reasoning_text: &str,
        plan_stream_text: &str,
        turn_running: bool,
    ) -> Option<String> {
        if turn_running {
            if let Some(started_at) = self.stream_started_at {
                let combined = format!("{agent_stream_text}{reasoning_text}{plan_stream_text}");
                let estimated_tokens = estimate_tokens_from_text(&combined);
                return format_tokens_per_second(streaming_tokens_per_second(
                    estimated_tokens,
                    started_at,
                    now,
                ));
            }
            return None;
        }

        self.last_completed_tokens_per_second
            .map(|tokens_per_second| format!("{tokens_per_second} t/s"))
    }
}

#[cfg(test)]
#[path = "token_throughput_tests.rs"]
mod tests;
