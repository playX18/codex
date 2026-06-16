use super::*;
use crate::test_backend::VT100Backend;
use ratatui::Terminal;

#[test]
fn step_card_renders_header_and_options() {
    let options = [
        StepCardOption {
            label: "Sign in with ChatGPT",
            description: Some("Usage included with paid plans"),
        },
        StepCardOption {
            label: "Provide your own API key",
            description: Some("Pay for what you use"),
        },
    ];
    let header = render_step_header("Sign in to Codex", Some("Choose how to authenticate"));

    let mut terminal =
        Terminal::new(VT100Backend::new(/*width*/ 60, /*height*/ 14)).expect("terminal");
    terminal
        .draw(|frame| {
            render_step_card_content(
                frame.area(),
                frame.buffer_mut(),
                "Sign in",
                header,
                Vec::new(),
                Some((&options, 0)),
            );
        })
        .expect("draw");

    insta::assert_snapshot!(terminal.backend());
}

#[test]
fn render_step_header_includes_subtitle() {
    let lines = render_step_header("Title", Some("Subtitle"));
    assert_eq!(lines.len(), 2);
}
