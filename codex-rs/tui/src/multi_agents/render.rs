//! Visual polish for multi-agent history rows and picker entries.

use ratatui::style::Stylize as _;
use ratatui::text::Span;

/// Status dot for `/agent` picker rows.
pub(crate) fn agent_picker_status_dot_spans(
    is_running: bool,
    is_closed: bool,
) -> Vec<Span<'static>> {
    let dot = if is_closed {
        "•".dim()
    } else if is_running {
        "•".green().bold()
    } else {
        "•".green()
    };
    vec![dot, " ".into()]
}

/// Title bullet prefix for collab history rows.
pub(crate) fn collab_title_bullet(is_active: bool) -> Span<'static> {
    if is_active {
        "• ".green()
    } else {
        "• ".dim()
    }
}

/// Role badge suffix such as `[explorer]`.
pub(crate) fn agent_role_badge(role: &str) -> Span<'static> {
    format!("[{role}]").magenta()
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
