//! Animated shutdown placeholder for the chat composer.

use std::time::Duration;

use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::motion::MotionMode;
use crate::motion::shimmer_text;
use crate::tui::FrameRequester;

const SHUTDOWN_FRAME_INTERVAL: Duration = Duration::from_millis(100);

/// Builds the shimmering shutdown placeholder and schedules follow-up frames when animated.
pub(crate) fn shutdown_placeholder_line(
    animations_enabled: bool,
    frame_requester: Option<&FrameRequester>,
) -> Line<'static> {
    let motion_mode = MotionMode::from_animations_enabled(animations_enabled);
    if matches!(motion_mode, MotionMode::Animated)
        && let Some(requester) = frame_requester
    {
        requester.schedule_frame_in(SHUTDOWN_FRAME_INTERVAL);
    }

    let mut spans: Vec<Span<'static>> = shimmer_text("Shutting down", motion_mode);
    spans.push("…".to_string().dim());
    Line::from(spans)
}

#[cfg(test)]
#[path = "shutdown_feedback_tests.rs"]
mod tests;
