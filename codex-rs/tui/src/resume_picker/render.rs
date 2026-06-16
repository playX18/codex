//! Visual polish helpers for the resume/session picker.

use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::history_cell::with_border;
use crate::motion::MotionMode;
use crate::motion::shimmer_text;
use crate::render::list_progress::list_progress_bar;

/// Selection marker with a cyan accent bar when selected.
pub(crate) fn selection_marker(is_selected: bool, is_expanded: bool) -> Span<'static> {
    match (is_selected, is_expanded) {
        (true, true) => "▌⌄ ".cyan().bold(),
        (true, false) => "▌❯ ".cyan().bold(),
        (false, _) => "  ".into(),
    }
}

/// Footer scroll progress as an ASCII bar plus dim fraction label.
pub(crate) fn picker_footer_progress_line(
    position: usize,
    total: usize,
    total_display: &str,
    percent: u8,
    bar_width: usize,
) -> Line<'static> {
    let current = if total == 0 {
        0
    } else {
        position.clamp(1, total)
    };
    let mut line = list_progress_bar(current, total.max(1), bar_width);
    line.spans.push(" ".into());
    line.spans
        .push(format!("{position} / {total_display} · {percent}%").dim());
    line
}

/// Empty-state card lines for the picker list.
pub(crate) fn empty_state_card_lines() -> Vec<Line<'static>> {
    with_border(vec![
        "⌁".dim().into(),
        "".into(),
        "No sessions yet".bold().into(),
        "Press Esc to start a new session".dim().into(),
    ])
}

/// Shimmering loading overlay message for transcript fetch.
pub(crate) fn transcript_loading_message(animations_enabled: bool) -> Line<'static> {
    let motion_mode = MotionMode::from_animations_enabled(animations_enabled);
    let spans = shimmer_text("Loading transcript", motion_mode);
    if spans.is_empty() {
        "Loading transcript…".bold().into()
    } else {
        let mut line_spans = spans;
        line_spans.push("…".dim());
        Line::from(line_spans)
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
