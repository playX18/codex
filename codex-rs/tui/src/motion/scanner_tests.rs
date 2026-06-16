use std::time::Instant;

use pretty_assertions::assert_eq;

use super::*;

#[test]
fn scanner_state_moves_forward_then_backward() {
    let width = 5u8;
    let forward = get_scanner_state(0, width, HOLD_START_FRAMES, HOLD_END_FRAMES);
    assert_eq!(forward.active_position, 0);
    assert!(forward.is_moving_forward);

    let end = get_scanner_state(4, width, HOLD_START_FRAMES, HOLD_END_FRAMES);
    assert_eq!(end.active_position, 4);

    let backward_start = 5 + HOLD_END_FRAMES;
    let backward = get_scanner_state(backward_start, width, HOLD_START_FRAMES, HOLD_END_FRAMES);
    assert_eq!(backward.active_position, 3);
    assert!(!backward.is_moving_forward);
}

#[test]
fn scanner_spans_produce_expected_width() {
    let start = Instant::now();
    let spans = scanner_spans(
        Some(start),
        MotionMode::Animated,
        /*width*/ 5,
        ScannerStyle::Diamonds,
    );
    assert_eq!(spans.len(), 5);
}

#[test]
fn reduced_motion_scanner_is_static_bullet() {
    let spans = scanner_spans(
        /*start_time*/ None,
        MotionMode::Reduced,
        /*width*/ 5,
        ScannerStyle::Diamonds,
    );
    assert_eq!(spans, vec!["•".dim()]);
}
