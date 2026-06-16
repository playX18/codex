//! Statusline helpers for token throughput and models.dev cost estimates.

use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;

use codex_models_dev::MODELS_DEV_CACHE_FILE;
use codex_models_dev::ModelsDevModelCost;
use codex_models_dev::TokenBillableUsage;
use codex_models_dev::estimate_cost_usd;
use codex_models_dev::format_cost_estimate_usd;
use codex_models_dev::format_turn_cost_estimate_usd;
use codex_models_dev::lookup_model_cost;

use super::*;
use crate::bottom_pane::StatusLineItem;
use crate::token_usage::TokenUsage;

const STATUS_LINE_THROUGHPUT_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

impl ChatWidget {
    pub(super) fn status_line_includes(&self, item: StatusLineItem) -> bool {
        self.configured_status_line_items().iter().any(|id| {
            StatusLineItem::from_str(id)
                .ok()
                .is_some_and(|configured| configured == item)
        })
    }

    pub(super) fn maybe_refresh_status_line_throughput(&mut self, now: Instant) {
        if !self.status_line_includes(StatusLineItem::TokenThroughput)
            || !self.turn_lifecycle.agent_turn_running
        {
            return;
        }

        let should_refresh = self
            .status_line_throughput_refresh_at
            .is_none_or(|last_refresh| {
                now.saturating_duration_since(last_refresh)
                    >= STATUS_LINE_THROUGHPUT_REFRESH_INTERVAL
            });
        if !should_refresh {
            return;
        }

        self.status_line_throughput_refresh_at = Some(now);
        self.refresh_status_line();
    }

    pub(super) fn record_statusline_stream_activity(&mut self) {
        if self.status_line_includes(StatusLineItem::TokenThroughput) {
            self.token_throughput.note_stream_activity(Instant::now());
        }
    }

    pub(super) fn status_line_token_throughput_value(&self, now: Instant) -> Option<String> {
        let agent_stream_text = self
            .stream_controller
            .as_ref()
            .map(StreamController::streamed_source)
            .unwrap_or_default();
        let plan_stream_text = self
            .plan_stream_controller
            .as_ref()
            .map(PlanStreamController::streamed_source)
            .unwrap_or(self.transcript.plan_delta_buffer.as_str());
        self.token_throughput.display_value(
            now,
            agent_stream_text,
            self.reasoning_buffer.as_str(),
            plan_stream_text,
            self.turn_lifecycle.agent_turn_running,
        )
    }

    pub(super) fn status_line_session_cost_estimate_value(&self) -> Option<String> {
        let cost_usd = self.estimate_session_cost_usd()?;
        if cost_usd <= 0.0 {
            return None;
        }
        Some(format_cost_estimate_usd(cost_usd))
    }

    pub(super) fn status_line_turn_cost_estimate_value(&self) -> Option<String> {
        let cost_usd = self.token_throughput.last_turn_cost_usd()?;
        if cost_usd <= 0.0 {
            return None;
        }
        Some(format_turn_cost_estimate_usd(cost_usd))
    }

    pub(super) fn finalize_turn_cost_estimate(&mut self) {
        let turn_cost_usd = self.estimate_last_turn_cost_usd();
        let started_at = self
            .turn_lifecycle
            .goal_status_active_turn_started_at
            .unwrap_or_else(Instant::now);
        let completed_at = Instant::now();
        let last_usage = self
            .token_info
            .as_ref()
            .map(|info| info.last_token_usage.clone())
            .unwrap_or_default();
        self.token_throughput.on_turn_completed(
            started_at,
            completed_at,
            &last_usage,
            turn_cost_usd,
        );
        self.status_line_throughput_refresh_at = None;
    }

    fn lookup_current_model_cost(&self) -> Option<ModelsDevModelCost> {
        let cache_path = self.config.codex_home.join(MODELS_DEV_CACHE_FILE);
        lookup_model_cost(
            &cache_path,
            self.config.model_provider_id.as_str(),
            self.current_model(),
        )
        .ok()?
    }

    fn estimate_session_cost_usd(&self) -> Option<f64> {
        let cost = self.lookup_current_model_cost()?;
        let usage = billable_usage_from_token_usage(&self.token_info.as_ref()?.total_token_usage);
        if usage.is_zero() {
            return None;
        }
        Some(estimate_cost_usd(usage, &cost))
    }

    fn estimate_last_turn_cost_usd(&self) -> Option<f64> {
        let cost = self.lookup_current_model_cost()?;
        let end = billable_usage_from_token_usage(&self.token_info.as_ref()?.total_token_usage);
        let start = billable_usage_from_token_usage(self.token_throughput.turn_usage_at_start()?);
        let delta = end.subtract(start);
        if delta.is_zero() {
            return None;
        }
        Some(estimate_cost_usd(delta, &cost))
    }
}

fn billable_usage_from_token_usage(usage: &TokenUsage) -> TokenBillableUsage {
    TokenBillableUsage {
        non_cached_input_tokens: usage.non_cached_input(),
        cached_input_tokens: usage.cached_input(),
        output_tokens: usage.output_tokens,
        reasoning_output_tokens: usage.reasoning_output_tokens,
    }
}
