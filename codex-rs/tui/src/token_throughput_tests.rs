use super::*;
use pretty_assertions::assert_eq;
use std::time::Duration;
use std::time::Instant;

#[test]
fn estimate_tokens_from_text_uses_chars_div_four() {
    assert_eq!(estimate_tokens_from_text(""), 0);
    assert_eq!(estimate_tokens_from_text("abcd"), 1);
    assert_eq!(estimate_tokens_from_text("abcde"), 2);
}

#[test]
fn streaming_tokens_per_second_waits_for_minimum_elapsed() {
    let started_at = Instant::now() - Duration::from_millis(100);
    assert_eq!(
        streaming_tokens_per_second(100, started_at, Instant::now()),
        None
    );

    let started_at = Instant::now() - Duration::from_millis(600);
    let tokens_per_second =
        streaming_tokens_per_second(120, started_at, Instant::now()).expect("tps");
    assert!(tokens_per_second > 100.0);
}

#[test]
fn completed_tokens_per_second_uses_output_and_reasoning() {
    let started_at = Instant::now() - Duration::from_secs(2);
    let completed_at = started_at + Duration::from_secs(2);
    let tokens_per_second = completed_tokens_per_second(
        /*output_tokens*/ 100,
        /*reasoning_tokens*/ 100,
        started_at,
        completed_at,
    )
    .expect("tps");
    assert_eq!(tokens_per_second, 100.0);
}

#[test]
fn format_tokens_per_second_rounds_and_clamps_low_values() {
    assert_eq!(
        format_tokens_per_second(Some(0.4)),
        Some("<1 t/s".to_string())
    );
    assert_eq!(
        format_tokens_per_second(Some(42.6)),
        Some("43 t/s".to_string())
    );
}

#[test]
fn display_value_shows_live_then_last_completed() {
    let mut state = TokenThroughputState::default();
    let now = Instant::now();
    let started_at = now - Duration::from_secs(1);
    state.stream_started_at = Some(started_at);
    assert_eq!(
        state.display_value(
            now,
            "a".repeat(400).as_str(),
            "",
            "",
            /*turn_running*/ true,
        ),
        Some("100 t/s".to_string())
    );

    state.on_turn_completed(
        now - Duration::from_secs(2),
        now,
        &TokenUsage {
            output_tokens: 80,
            reasoning_output_tokens: 20,
            ..TokenUsage::default()
        },
        Some(0.01),
    );
    assert_eq!(
        state.display_value(now, "", "", "", /*turn_running*/ false),
        Some("50 t/s".to_string())
    );
}
