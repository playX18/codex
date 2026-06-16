use super::*;
use pretty_assertions::assert_eq;

#[test]
fn shutdown_placeholder_reduced_motion_is_plain_text() {
    let line = shutdown_placeholder_line(/*animations_enabled*/ false, None);
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    assert_eq!(text, "Shutting down…");
}
