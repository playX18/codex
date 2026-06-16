//! Top-right transient notification panel with a colored left accent border.

use std::time::Duration;
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use crate::style::user_message_style;

const DEFAULT_DURATION: Duration = Duration::from_secs(5);
const MAX_PANEL_WIDTH: u16 = 60;
const MIN_MARGIN: u16 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToastVariant {
    Info,
    #[cfg(test)]
    Error,
}

impl ToastVariant {
    fn accent_span(self) -> Span<'static> {
        match self {
            Self::Info => "┃".cyan(),
            #[cfg(test)]
            Self::Error => "┃".red(),
        }
    }
}

pub(crate) struct Toast {
    pub(crate) title: Option<String>,
    pub(crate) message: Line<'static>,
    pub(crate) variant: ToastVariant,
    pub(crate) expires_at: Instant,
}

impl Toast {
    pub(crate) fn new(
        title: Option<String>,
        message: Line<'static>,
        variant: ToastVariant,
        duration: Duration,
    ) -> Self {
        let expires_at = Instant::now()
            .checked_add(duration)
            .unwrap_or_else(Instant::now);
        Self {
            title,
            message,
            variant,
            expires_at,
        }
    }

    pub(crate) fn visible(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

pub(crate) fn show_toast(
    slot: &mut Option<Toast>,
    title: Option<String>,
    message: Line<'static>,
    variant: ToastVariant,
    duration: Option<Duration>,
) {
    prune_toast(slot);
    *slot = Some(Toast::new(
        title,
        message,
        variant,
        duration.unwrap_or(DEFAULT_DURATION),
    ));
}

pub(crate) fn prune_toast(slot: &mut Option<Toast>) {
    if slot.as_ref().is_some_and(|toast| !toast.visible()) {
        *slot = None;
    }
}

pub(crate) fn render_toast(area: Rect, buf: &mut Buffer, toast: &Toast) {
    if area.width < MIN_MARGIN || area.height < 4 {
        return;
    }

    let panel_width = area.width.saturating_sub(4).min(MAX_PANEL_WIDTH).max(20);
    let panel_height = toast_height(toast);
    let x = area.x + area.width.saturating_sub(panel_width + 2);
    let y = area.y.saturating_add(1);
    let panel_area = Rect::new(x, y, panel_width, panel_height);
    if panel_area.bottom() > area.bottom() {
        return;
    }

    Clear.render(panel_area, buf);
    Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_type(BorderType::Plain)
        .border_style(user_message_style())
        .style(user_message_style())
        .render(panel_area, buf);

    let accent_x = panel_area.x;
    for row in 0..panel_area.height {
        buf[(accent_x, panel_area.y + row)].set_symbol("┃");
        buf[(accent_x, panel_area.y + row)].set_style(toast.variant.accent_span().style);
    }

    let inner = panel_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let mut lines = Vec::new();
    if let Some(title) = &toast.title {
        lines.push(Line::from(title.clone().bold()));
    }
    lines.push(toast.message.clone());
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .style(user_message_style())
        .render(inner, buf);
}

fn toast_height(toast: &Toast) -> u16 {
    let base = if toast.title.is_some() { 3 } else { 2 };
    base.max(2)
}

#[cfg(test)]
#[path = "toast_tests.rs"]
mod tests;
