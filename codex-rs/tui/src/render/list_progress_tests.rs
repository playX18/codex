use super::*;
use pretty_assertions::assert_eq;

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn list_progress_bar_renders_fill_without_label() {
    let line = list_progress_bar(/*current*/ 3, /*total*/ 10, /*bar_width*/ 10);
    assert_eq!(line_text(&line), "███░░░░░░░");
}

#[test]
fn list_progress_bar_clamps_current_to_total() {
    let line = list_progress_bar(/*current*/ 99, /*total*/ 10, /*bar_width*/ 5);
    assert_eq!(line_text(&line), "█████");
}

#[test]
fn list_progress_bar_empty_when_total_zero() {
    let line = list_progress_bar(/*current*/ 1, /*total*/ 0, /*bar_width*/ 8);
    assert_eq!(line_text(&line), "░░░░░░░░");
}
