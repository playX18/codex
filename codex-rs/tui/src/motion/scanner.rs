//! Knight Rider-style bidirectional scanner animation.
//!
//! Ported from MiMo-Code's `ui/spinner.ts` scanner state machine, adapted for
//! ratatui spans with ANSI cyan trail falloff.

use std::time::Instant;

use ratatui::style::Stylize;
use ratatui::text::Span;

use super::MotionMode;

const MS_PER_FRAME: u64 = 120;
const HOLD_START_FRAMES: u32 = 8;
const HOLD_END_FRAMES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScannerStyle {
    Diamonds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScannerState {
    active_position: u8,
    is_holding: bool,
    hold_progress: u32,
    hold_total: u32,
    movement_progress: u32,
    movement_total: u32,
    is_moving_forward: bool,
}

fn get_scanner_state(
    frame_index: u32,
    total_chars: u8,
    hold_start: u32,
    hold_end: u32,
) -> ScannerState {
    let width = u32::from(total_chars);
    let forward_frames = width;
    let backward_frames = width.saturating_sub(1);

    if frame_index < forward_frames {
        return ScannerState {
            active_position: frame_index as u8,
            is_holding: false,
            hold_progress: 0,
            hold_total: 0,
            movement_progress: frame_index,
            movement_total: forward_frames,
            is_moving_forward: true,
        };
    }
    let after_forward = frame_index - forward_frames;
    if after_forward < hold_end {
        return ScannerState {
            active_position: total_chars.saturating_sub(1),
            is_holding: true,
            hold_progress: after_forward,
            hold_total: hold_end,
            movement_progress: 0,
            movement_total: 0,
            is_moving_forward: true,
        };
    }
    let after_hold_end = after_forward - hold_end;
    if after_hold_end < backward_frames {
        let backward_index = after_hold_end;
        return ScannerState {
            active_position: total_chars
                .saturating_sub(2)
                .saturating_sub(backward_index as u8),
            is_holding: false,
            hold_progress: 0,
            hold_total: 0,
            movement_progress: backward_index,
            movement_total: backward_frames,
            is_moving_forward: false,
        };
    }
    let hold_start_progress = after_hold_end - backward_frames;
    ScannerState {
        active_position: 0,
        is_holding: true,
        hold_progress: hold_start_progress,
        hold_total: hold_start,
        movement_progress: 0,
        movement_total: 0,
        is_moving_forward: false,
    }
}

fn calculate_color_index(char_index: u8, trail_length: u8, state: ScannerState) -> i8 {
    let directional_distance = if state.is_moving_forward {
        i16::from(state.active_position) - i16::from(char_index)
    } else {
        i16::from(char_index) - i16::from(state.active_position)
    };

    if state.is_holding {
        return i8::try_from(
            directional_distance + i16::try_from(state.hold_progress).unwrap_or(i16::MAX),
        )
        .unwrap_or(-1);
    }

    if (1..i16::from(trail_length)).contains(&directional_distance) {
        return i8::try_from(directional_distance).unwrap_or(-1);
    }

    if directional_distance == 0 { 0 } else { -1 }
}

fn scanner_char(style: ScannerStyle, color_index: i8) -> char {
    match style {
        ScannerStyle::Diamonds => match color_index {
            0 => '◆',
            1 => '⬩',
            2 => '⬪',
            3.. => '·',
            _ => '·',
        },
    }
}

fn trail_span(ch: char, color_index: i8) -> Span<'static> {
    let text = ch.to_string();
    match color_index {
        0 => text.cyan(),
        1 => text.cyan().dim(),
        2 => text.dim(),
        _ => text.dim(),
    }
}

fn total_cycle_frames(width: u8) -> u32 {
    u32::from(width) + HOLD_END_FRAMES + u32::from(width.saturating_sub(1)) + HOLD_START_FRAMES
}

fn frame_index_at(start_time: Option<Instant>, width: u8) -> u32 {
    let elapsed_ms = start_time
        .map(|start| start.elapsed().as_millis() as u64)
        .unwrap_or(0);
    let cycle = u64::from(total_cycle_frames(width));
    (elapsed_ms / MS_PER_FRAME % cycle.max(1)) as u32
}

/// Renders a short Knight Rider scanner strip for busy status rows.
pub(crate) fn scanner_spans(
    start_time: Option<Instant>,
    motion_mode: MotionMode,
    width: u8,
    style: ScannerStyle,
) -> Vec<Span<'static>> {
    let width = width.max(1);
    match motion_mode {
        MotionMode::Reduced => return vec!["•".dim()],
        MotionMode::Animated => {}
    }

    let frame_index = frame_index_at(start_time, width);
    let state = get_scanner_state(frame_index, width, HOLD_START_FRAMES, HOLD_END_FRAMES);
    let trail_length = 4u8;

    (0..width)
        .map(|char_index| {
            let color_index = calculate_color_index(char_index, trail_length, state);
            let ch = scanner_char(style, color_index);
            trail_span(ch, color_index)
        })
        .collect()
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod tests;
