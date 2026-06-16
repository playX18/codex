use super::*;

#[test]
fn selection_marker_selected_includes_accent_bar() {
    let marker = selection_marker(/*is_selected*/ true, /*is_expanded*/ false);
    assert!(marker.content.contains('▌'));
}

#[test]
fn empty_state_card_has_border() {
    let lines = empty_state_card_lines();
    assert!(
        lines
            .first()
            .is_some_and(|line| line.to_string().contains('╭'))
    );
}
