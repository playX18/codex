use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::text::Line;

use super::*;

#[test]
fn toast_expires_after_duration() {
    let toast = Toast::new(
        /*title*/ None,
        Line::from("hello"),
        ToastVariant::Info,
        Duration::from_millis(1),
    );
    std::thread::sleep(Duration::from_millis(2));
    assert!(!toast.visible());
}

#[test]
fn render_info_toast_snapshot() {
    let toast = Toast::new(
        Some("Companion".to_string()),
        Line::from("Enable terminal images for companion mode.".dim()),
        ToastVariant::Info,
        Duration::from_secs(60),
    );
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).expect("terminal");
    terminal
        .draw(|frame| render_toast(frame.area(), frame.buffer_mut(), &toast))
        .expect("draw");
    insta::assert_snapshot!(terminal.backend());
}

#[test]
fn render_error_toast_snapshot() {
    let toast = Toast::new(
        /*title*/ None,
        Line::from("Something went wrong.".red()),
        ToastVariant::Error,
        Duration::from_secs(60),
    );
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).expect("terminal");
    terminal
        .draw(|frame| render_toast(frame.area(), frame.buffer_mut(), &toast))
        .expect("draw");
    insta::assert_snapshot!(terminal.backend());
}
