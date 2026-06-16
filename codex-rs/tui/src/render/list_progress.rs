//! ASCII list progress bars for scrollable picker footers and similar surfaces.

use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;

const PROGRESS_BAR_FILLED: &str = "█";
const PROGRESS_BAR_EMPTY: &str = "░";

/// Builds a progress bar line with a cyan fill and dim empty track.
pub(crate) fn list_progress_bar(current: usize, total: usize, bar_width: usize) -> Line<'static> {
    let bar_width = bar_width.max(1);
    let filled = progress_filled(current, total, bar_width);
    Line::from(vec![
        Span::from(PROGRESS_BAR_FILLED.repeat(filled)).cyan(),
        Span::from(PROGRESS_BAR_EMPTY.repeat(bar_width.saturating_sub(filled))).dim(),
    ])
}

fn progress_filled(current: usize, total: usize, bar_width: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let current = current.clamp(1, total);
    let ratio = current as f64 / total as f64;
    let filled = (ratio * bar_width as f64).round() as usize;
    filled.clamp(0, bar_width)
}

#[cfg(test)]
#[path = "list_progress_tests.rs"]
mod tests;
