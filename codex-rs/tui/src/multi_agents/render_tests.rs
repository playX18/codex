use super::*;
use pretty_assertions::assert_eq;

#[test]
fn closed_agent_picker_dot_is_dim() {
    let spans = agent_picker_status_dot_spans(/*is_running*/ false, /*is_closed*/ true);
    assert_eq!(spans[0].content, "•");
}

#[test]
fn running_agent_picker_dot_is_green() {
    let spans = agent_picker_status_dot_spans(/*is_running*/ true, /*is_closed*/ false);
    assert_eq!(spans[0].content, "•");
}
