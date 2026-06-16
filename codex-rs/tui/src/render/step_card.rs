//! Shared bordered step-card rendering for onboarding and picker surfaces.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;

const STEP_CARD_INSET_V: u16 = 1;
const STEP_CARD_INSET_H: u16 = 2;

/// One selectable row in a step card.
pub(crate) struct StepCardOption<'a> {
    pub label: &'a str,
    pub description: Option<&'a str>,
}

/// Bold title with an optional dim subtitle on the following line.
pub(crate) fn render_step_header(title: &str, subtitle: Option<&str>) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(title.to_string().bold())];
    if let Some(subtitle) = subtitle {
        lines.push(subtitle.to_string().dim().into());
    }
    lines
}

/// Selection rows using the shared `› N.` prefix and cyan highlight.
pub(crate) fn render_step_options(
    options: &[StepCardOption<'_>],
    highlighted_idx: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (idx, option) in options.iter().enumerate() {
        if idx > 0 {
            lines.push("".into());
        }
        let is_selected = idx == highlighted_idx;
        let prefix = if is_selected {
            format!("› {}. ", idx + 1)
        } else {
            format!("  {}. ", idx + 1)
        };
        let row = if is_selected {
            Line::from(vec![prefix.cyan().dim(), option.label.to_string().cyan()])
        } else {
            Line::from(format!("{prefix}{}", option.label))
        };
        lines.push(row);
        if let Some(description) = option.description {
            let indent = if is_selected {
                Line::from(format!("     {description}")).cyan().dim()
            } else {
                format!("     {description}").dim().into()
            };
            lines.push(indent);
        }
    }
    lines
}

/// Paint a rounded bordered step card and return the inset content area.
pub(crate) fn render_step_card_surface(area: Rect, buf: &mut Buffer, title: &str) -> Rect {
    if area.is_empty() {
        return area;
    }
    Block::default()
        .title(title.bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .style(user_message_style())
        .render(area, buf);
    area.inset(Insets::vh(STEP_CARD_INSET_V, STEP_CARD_INSET_H))
}

/// Render header, body lines, and optional options inside a step card.
pub(crate) fn render_step_card_content(
    area: Rect,
    buf: &mut Buffer,
    card_title: &str,
    header: Vec<Line<'static>>,
    body: Vec<Line<'static>>,
    options: Option<(&[StepCardOption<'_>], usize)>,
) {
    let content_area = render_step_card_surface(area, buf, card_title);
    if content_area.is_empty() {
        return;
    }

    let mut lines = header;
    if !body.is_empty() {
        if !lines.is_empty() {
            lines.push("".into());
        }
        lines.extend(body);
    }
    if let Some((options, highlighted_idx)) = options {
        if !lines.is_empty() {
            lines.push("".into());
        }
        lines.extend(render_step_options(options, highlighted_idx));
    }

    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .render(content_area, buf);
}

#[cfg(test)]
#[path = "step_card_tests.rs"]
mod tests;
