use std::fs::File;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::common::gif::GifTool;
use crate::dc_control;
use crate::platform::Rect;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordStatus {
    #[default]
    Init,
    Recording,
    Pause,
    Stop,
    Playing,
}

#[derive(Clone, Copy, Debug)]
pub struct GifRecordOptions {
    pub fps: u32,
    pub max_duration: Duration,
    pub max_cache_bytes: usize,
}

impl Default for GifRecordOptions {
    fn default() -> Self {
        Self {
            fps: 10,
            max_duration: Duration::from_secs(5 * 60),
            max_cache_bytes: 150 * 1024 * 1024,
        }
    }
}

pub fn record_region_to_file(
    desktop_bounds: Rect,
    region: Rect,
    path: impl AsRef<Path>,
    options: GifRecordOptions,
) -> bool {
    if region.w <= 0 || region.h <= 0 || region.w > u16::MAX as i32 || region.h > u16::MAX as i32 {
        return false;
    }

    let fps = options.fps.max(1);
    let frame_delay = Duration::from_millis((1000 / fps).max(1) as u64);
    let frame_budget = (options.max_duration.as_millis() / frame_delay.as_millis().max(1)) as usize;
    let mut gif = GifTool::new(region.w as u16, region.h as u16)
        .with_limits(options.max_cache_bytes, options.max_duration);

    let started = Instant::now();
    for _ in 0..frame_budget {
        let Some(frame) = dc_control::capture_region(desktop_bounds, region) else {
            return false;
        };
        if !gif.add_frame(&frame.pixels, frame_delay) {
            break;
        }
        let elapsed = started.elapsed();
        if elapsed >= options.max_duration {
            break;
        }
        thread::sleep(frame_delay);
    }

    let Ok(file) = File::create(path) else {
        return false;
    };
    gif.encode(file).is_ok()
}
