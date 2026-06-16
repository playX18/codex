use std::path::PathBuf;

use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::key_hint::KeyBindingListExt;
use crate::onboarding::keys;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::render::step_card::StepCardOption;
use crate::render::step_card::render_step_card_content;
use crate::render::step_card::render_step_header;

use super::onboarding_screen::StepState;
pub(crate) struct TrustDirectoryWidget {
    pub cwd: PathBuf,
    pub trust_target: PathBuf,
    pub should_quit: bool,
    pub selection: Option<TrustDirectorySelection>,
    pub highlighted: TrustDirectorySelection,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustDirectorySelection {
    Trust,
    Quit,
}

impl WidgetRef for &TrustDirectoryWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let highlighted_idx = match self.highlighted {
            TrustDirectorySelection::Trust => 0,
            TrustDirectorySelection::Quit => 1,
        };

        let cwd = self.cwd.to_string_lossy().to_string();
        let mut body = vec![Line::from(vec![
            "You are in ".into(),
            cwd.cyan(),
            " with ".into(),
            "Codex".magenta(),
        ])];

        if self.cwd != self.trust_target {
            body.push("".into());
            body.push(
                Line::from(vec![
                    "Note: You're in a subdirectory of a Git project. Trusting applies to the repository root: "
                        .dim(),
                    self.trust_target.display().to_string().cyan(),
                ]),
            );
        }

        body.push("".into());
        body.push(
            "Do you trust the contents of this directory? Working with untrusted contents comes with higher risk of prompt injection."
                .dim()
                .into(),
        );
        body.push(
            "Trusting the directory allows project-local config, hooks, and exec policies to load."
                .dim()
                .into(),
        );
        body.push("".into());
        body.push("Project-local config loads after trust.".dim().into());

        let options = [
            StepCardOption {
                label: "Yes, continue",
                description: None,
            },
            StepCardOption {
                label: "No, quit",
                description: None,
            },
        ];

        let header = render_step_header("Trust this directory", None);
        render_step_card_content(
            area,
            buf,
            "Trust",
            header,
            body,
            Some((&options, highlighted_idx)),
        );

        if let Some(error) = &self.error {
            let error_area = Rect {
                y: area.y.saturating_add(area.height.saturating_sub(3)),
                height: 2,
                ..area
            };
            if !error_area.is_empty() {
                Paragraph::new(error.clone())
                    .red()
                    .wrap(Wrap { trim: true })
                    .render(error_area, buf);
            }
        }
    }
}

impl KeyboardHandler for TrustDirectoryWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if keys::MOVE_UP.is_pressed(key_event) {
            self.highlighted = TrustDirectorySelection::Trust;
        } else if keys::MOVE_DOWN.is_pressed(key_event) {
            self.highlighted = TrustDirectorySelection::Quit;
        } else if keys::SELECT_FIRST.is_pressed(key_event) {
            self.handle_trust();
        } else if keys::SELECT_SECOND.is_pressed(key_event)
            || keys::QUIT.is_pressed(key_event)
            || keys::CANCEL.is_pressed(key_event)
        {
            self.handle_quit();
        } else if keys::CONFIRM.is_pressed(key_event) {
            match self.highlighted {
                TrustDirectorySelection::Trust => self.handle_trust(),
                TrustDirectorySelection::Quit => self.handle_quit(),
            }
        }
    }
}

impl StepStateProvider for TrustDirectoryWidget {
    fn get_step_state(&self) -> StepState {
        if self.selection.is_some() || self.should_quit {
            StepState::Complete
        } else {
            StepState::InProgress
        }
    }
}

impl TrustDirectoryWidget {
    fn handle_trust(&mut self) {
        self.highlighted = TrustDirectorySelection::Trust;
        self.error = None;
        self.selection = Some(TrustDirectorySelection::Trust);
    }

    fn handle_quit(&mut self) {
        self.highlighted = TrustDirectorySelection::Quit;
        self.should_quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
}

#[cfg(test)]
mod tests {
    use crate::test_backend::VT100Backend;

    use super::*;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyEventKind;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn widget(error: Option<String>) -> TrustDirectoryWidget {
        TrustDirectoryWidget {
            cwd: PathBuf::from("/workspace/project"),
            trust_target: PathBuf::from("/workspace/project"),
            should_quit: false,
            selection: None,
            highlighted: TrustDirectorySelection::Trust,
            error,
        }
    }

    #[test]
    fn release_event_does_not_change_selection() {
        let mut widget = TrustDirectoryWidget {
            cwd: PathBuf::from("."),
            trust_target: PathBuf::from("."),
            should_quit: false,
            selection: None,
            highlighted: TrustDirectorySelection::Quit,
            error: None,
        };

        let release = KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
        };
        widget.handle_key_event(release);
        assert_eq!(widget.selection, None);

        let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        widget.handle_key_event(press);
        assert!(widget.should_quit);
    }

    #[test]
    fn renders_snapshot_for_git_repo() {
        let widget = widget(/*error*/ None);

        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 70, /*height*/ 14)).expect("terminal");
        terminal
            .draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))
            .expect("draw");

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn renders_snapshot_for_trust_error() {
        let widget = widget(Some(
            "Failed to set trust for /workspace/project: config/batchWrite failed in TUI: Invalid configuration: features.fast_mode=true is not supported; allowed set [fast_mode=false]"
                .to_string(),
        ));

        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 70, /*height*/ 18)).expect("terminal");
        terminal
            .draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))
            .expect("draw");

        insta::assert_snapshot!(terminal.backend());
    }
}
