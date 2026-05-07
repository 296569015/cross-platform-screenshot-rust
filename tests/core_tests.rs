use cross_platform_screenshot_rust::core::{
    Annotation, AnnotationModel, AppEvent, AppState, FreehandAnnotation,
    LongScreenshotStitchOptions, LongScreenshotStitcher, ScreenshotStateMachine,
};
use cross_platform_screenshot_rust::platform::{Color, PointF};

fn make_source_image(width: i32, height: i32) -> Vec<u8> {
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let i = (y as usize * width as usize + x as usize) * 4;
            pixels[i] = ((x * 17 + y * 11) & 0xff) as u8;
            pixels[i + 1] = ((x * 7 + y * 23) & 0xff) as u8;
            pixels[i + 2] = ((x * 13 + y * 5) & 0xff) as u8;
            pixels[i + 3] = 255;
        }
    }
    pixels
}

fn make_repeating_source_image(width: i32, height: i32, period: i32) -> Vec<u8> {
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        let py = y % period;
        for x in 0..width {
            let i = (y as usize * width as usize + x as usize) * 4;
            pixels[i] = ((py * 41 + x * 13) & 0xff) as u8;
            pixels[i + 1] = ((py * 73 + x * 5) & 0xff) as u8;
            pixels[i + 2] = ((py * 29 + x * 17) & 0xff) as u8;
            pixels[i + 3] = 255;
        }
    }
    pixels
}

fn crop_rows(source: &[u8], width: i32, start_y: i32, height: i32) -> Vec<u8> {
    let mut rows = vec![0; width as usize * height as usize * 4];
    let row_bytes = width as usize * 4;
    for y in 0..height as usize {
        let src = (start_y as usize + y) * row_bytes;
        let dst = y * row_bytes;
        rows[dst..dst + row_bytes].copy_from_slice(&source[src..src + row_bytes]);
    }
    rows
}

#[test]
fn state_machine_matches_capture_flow() {
    let mut sm = ScreenshotStateMachine::default();
    assert_eq!(sm.current_state(), AppState::Idle);
    sm.transition(AppEvent::HotkeyTriggered);
    assert_eq!(sm.current_state(), AppState::Capturing);
    sm.transition(AppEvent::FrameAcquired);
    assert_eq!(sm.current_state(), AppState::Selecting);
    sm.transition(AppEvent::MouseUp);
    assert_eq!(sm.current_state(), AppState::Annotating);
    sm.transition(AppEvent::SaveRequested);
    assert_eq!(sm.current_state(), AppState::Saving);
    sm.transition(AppEvent::SaveComplete);
    assert_eq!(sm.current_state(), AppState::Idle);
}

#[test]
fn invalid_transition_stays_in_current_state() {
    let mut sm = ScreenshotStateMachine::default();
    sm.transition(AppEvent::MouseUp);
    assert_eq!(sm.current_state(), AppState::Idle);
}

#[test]
fn long_screenshot_stitches_overlapping_frames() {
    let width = 8;
    let source_height = 18;
    let frame_height = 8;
    let source = make_source_image(width, source_height);
    let options = LongScreenshotStitchOptions {
        min_overlap_rows: 2,
        max_overlap_rows: 8,
        min_append_rows: 1,
        reliable_match_score: 0.0,
        ..Default::default()
    };
    let mut stitcher = LongScreenshotStitcher::new(width, options);
    stitcher.start(&crop_rows(&source, width, 0, frame_height), frame_height);
    let r1 = stitcher.append(
        &crop_rows(&source, width, 5, frame_height),
        frame_height,
        true,
    );
    let r2 = stitcher.append(
        &crop_rows(&source, width, 10, frame_height),
        frame_height,
        true,
    );
    assert!(r1.appended);
    assert!(r1.reliable);
    assert_eq!(r1.overlap_rows, 3);
    assert!(r2.appended);
    assert!(r2.reliable);
    assert_eq!(r2.overlap_rows, 3);
    assert_eq!(stitcher.height(), source_height);
    assert_eq!(stitcher.pixels(), source);
}

#[test]
fn long_screenshot_handles_large_overlap_search_range() {
    let width = 20;
    let source_height = 420;
    let frame_height = 260;
    let scroll_rows = 113;
    let source = make_source_image(width, source_height);
    let options = LongScreenshotStitchOptions {
        min_overlap_rows: 40,
        max_overlap_rows: 240,
        min_append_rows: 1,
        reliable_match_score: 0.0,
        ..Default::default()
    };
    let mut stitcher = LongScreenshotStitcher::new(width, options);
    stitcher.start(&crop_rows(&source, width, 0, frame_height), frame_height);
    let result = stitcher.append(
        &crop_rows(&source, width, scroll_rows, frame_height),
        frame_height,
        true,
    );
    assert!(result.appended);
    assert!(result.reliable);
    assert_eq!(result.overlap_rows, frame_height - scroll_rows);
    assert_eq!(stitcher.height(), frame_height + scroll_rows);
    assert_eq!(
        stitcher.pixels(),
        crop_rows(&source, width, 0, stitcher.height())
    );
}

#[test]
fn long_screenshot_detects_duplicate_frame() {
    let width = 6;
    let frame_height = 7;
    let frame = make_source_image(width, frame_height);
    let options = LongScreenshotStitchOptions {
        min_overlap_rows: 2,
        max_overlap_rows: 7,
        ..Default::default()
    };
    let mut stitcher = LongScreenshotStitcher::new(width, options);
    stitcher.start(&frame, frame_height);
    let result = stitcher.append(&frame, frame_height, true);
    assert!(result.duplicate);
    assert!(!result.appended);
    assert_eq!(stitcher.height(), frame_height);
}

#[test]
fn long_screenshot_stops_on_unreliable_overlap() {
    let width = 8;
    let frame_height = 8;
    let first = make_source_image(width, frame_height);
    let mut unrelated = make_source_image(width, frame_height);
    for px in unrelated.chunks_exact_mut(4) {
        px[0] = 255 - px[0];
        px[1] = 255 - px[1];
        px[2] = 255 - px[2];
    }
    let options = LongScreenshotStitchOptions {
        min_overlap_rows: 2,
        max_overlap_rows: 7,
        min_append_rows: 1,
        append_on_unreliable_match: false,
        ..Default::default()
    };
    let mut stitcher = LongScreenshotStitcher::new(width, options);
    stitcher.start(&first, frame_height);
    let result = stitcher.append(&unrelated, frame_height, true);
    assert!(!result.appended);
    assert!(!result.duplicate);
    assert_eq!(stitcher.height(), frame_height);
}

#[test]
fn long_screenshot_rejects_ambiguous_repeating_overlap() {
    let width = 16;
    let source_height = 80;
    let frame_height = 40;
    let scroll_rows = 7;
    let source = make_repeating_source_image(width, source_height, 4);
    let options = LongScreenshotStitchOptions {
        min_overlap_rows: 12,
        max_overlap_rows: 39,
        min_append_rows: 1,
        reliable_match_score: 1.0,
        ambiguous_score_gap: 2.0,
        append_on_unreliable_match: false,
        ..Default::default()
    };
    let mut stitcher = LongScreenshotStitcher::new(width, options);
    stitcher.start(&crop_rows(&source, width, 0, frame_height), frame_height);
    let result = stitcher.append(
        &crop_rows(&source, width, scroll_rows, frame_height),
        frame_height,
        true,
    );
    assert!(!result.appended);
    assert!(!result.duplicate);
    assert!(!result.reliable);
    assert_eq!(stitcher.height(), frame_height);
}

#[test]
fn long_screenshot_prefers_larger_overlap_when_scores_are_close() {
    let width = 32;
    let source_height = 180;
    let frame_height = 120;
    let scroll_rows = 30;
    let source = make_repeating_source_image(width, source_height, scroll_rows);
    let mut next = crop_rows(&source, width, scroll_rows, frame_height);

    let row_bytes = width as usize * 4;
    for y in 60..90 {
        let i = y as usize * row_bytes;
        next[i] = next[i].saturating_add(1);
        next[i + 1] = next[i + 1].saturating_add(1);
        next[i + 2] = next[i + 2].saturating_add(1);
    }

    let options = LongScreenshotStitchOptions {
        min_overlap_rows: 50,
        max_overlap_rows: 119,
        min_append_rows: 1,
        duplicate_score: 0.0,
        reliable_match_score: 1.0,
        append_on_unreliable_match: true,
        ..Default::default()
    };
    let mut stitcher = LongScreenshotStitcher::new(width, options);
    stitcher.start(&crop_rows(&source, width, 0, frame_height), frame_height);
    let result = stitcher.append(&next, frame_height, true);

    assert!(result.appended);
    assert_eq!(result.overlap_rows, frame_height - scroll_rows);
    assert_eq!(result.appended_rows, scroll_rows);
}

#[test]
fn freehand_annotation_model_stores_points() {
    let mut model = AnnotationModel::default();
    model.add_annotation(Annotation::Freehand(FreehandAnnotation {
        points: vec![
            PointF { x: 3.0, y: 4.0 },
            PointF { x: 8.0, y: 9.0 },
            PointF { x: 13.0, y: 10.0 },
        ],
        color: Color::rgba(255, 68, 68, 255),
        thickness: 3.0,
    }));
    assert_eq!(model.count(), 1);
    let Annotation::Freehand(stored) = &model.annotations()[0] else {
        panic!("expected freehand annotation");
    };
    assert_eq!(stored.points.len(), 3);
    assert_eq!(stored.points[0].x, 3.0);
    assert_eq!(stored.points[2].y, 10.0);
    assert_eq!(stored.thickness, 3.0);
}
