use super::stitcher::{
    LongScreenshotStitchOptions, LongScreenshotStitchResult, LongScreenshotStitcher,
};
use super::types::{LongShotImage, LongShotRect};

pub const DEFAULT_LONG_SHOT_MAX_OUTPUT_HEIGHT: i32 = 16_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongShotSessionOptions {
    pub stitch: LongScreenshotStitchOptions,
    pub max_output_height: i32,
    pub trim_capture_dropout: bool,
}

impl LongShotSessionOptions {
    pub fn tuned_for_region_height(region_height: i32) -> Self {
        let mut stitch = LongScreenshotStitchOptions::default();
        stitch.min_append_rows = (region_height / 20).max(24);
        stitch.min_overlap_rows = (region_height - 1).min((region_height * 2 / 5).max(80));
        stitch.max_overlap_rows = 900.min((region_height - stitch.min_append_rows).max(1));
        stitch.reliable_match_score = 24.0;
        stitch.acceptable_match_score = 15.5;
        stitch.ambiguous_score_gap = 2.0;
        stitch.acceptable_score_gap = 0.4;
        Self {
            stitch,
            ..Default::default()
        }
    }
}

impl Default for LongShotSessionOptions {
    fn default() -> Self {
        Self {
            stitch: LongScreenshotStitchOptions::default(),
            max_output_height: DEFAULT_LONG_SHOT_MAX_OUTPUT_HEIGHT,
            trim_capture_dropout: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongShotStartError {
    InvalidRegion,
    InvalidFrame,
    FrameWidthMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongShotAppendStatus {
    NotActive,
    EmptyFrame,
    WidthMismatch,
    TooTall,
    SameScrollAlreadyAppended,
    Appended,
    Duplicate,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongShotAppendOutcome {
    pub status: LongShotAppendStatus,
    pub stitch: LongScreenshotStitchResult,
    pub scroll_seq: u64,
    pub output_width: i32,
    pub output_height: i32,
}

impl LongShotAppendOutcome {
    pub fn appended(&self) -> bool {
        self.status == LongShotAppendStatus::Appended
    }
}

#[derive(Clone, Debug)]
pub struct LongShotSession {
    active: bool,
    source_region: LongShotRect,
    stitcher: LongScreenshotStitcher,
    options: LongShotSessionOptions,
    scroll_seq: u64,
    last_appended_scroll_seq: u64,
}

impl Default for LongShotSession {
    fn default() -> Self {
        Self {
            active: false,
            source_region: LongShotRect::default(),
            stitcher: LongScreenshotStitcher::default(),
            options: LongShotSessionOptions::default(),
            scroll_seq: 0,
            last_appended_scroll_seq: 0,
        }
    }
}

impl LongShotSession {
    pub fn start(
        &mut self,
        source_region: LongShotRect,
        first_frame: LongShotImage,
        options: LongShotSessionOptions,
    ) -> Result<(), LongShotStartError> {
        if source_region.is_empty() {
            self.reset();
            return Err(LongShotStartError::InvalidRegion);
        }
        if !first_frame.is_valid_rgba() || first_frame.height <= 0 {
            self.reset();
            return Err(LongShotStartError::InvalidFrame);
        }
        if first_frame.width != source_region.w {
            self.reset();
            return Err(LongShotStartError::FrameWidthMismatch);
        }

        let mut first_frame = first_frame;
        if options.trim_capture_dropout {
            trim_image_trailing_capture_dropout(&mut first_frame);
        }
        if first_frame.height <= 0 {
            self.reset();
            return Err(LongShotStartError::InvalidFrame);
        }

        self.active = true;
        self.source_region = source_region;
        self.options = options;
        self.scroll_seq = 0;
        self.last_appended_scroll_seq = 0;
        self.stitcher = LongScreenshotStitcher::new(source_region.w, options.stitch);
        self.stitcher.start(&first_frame.pixels, first_frame.height);
        Ok(())
    }

    pub fn start_from_full_capture(
        &mut self,
        source_region: LongShotRect,
        full_capture: &LongShotImage,
        options: LongShotSessionOptions,
    ) -> Result<(), LongShotStartError> {
        let first_frame = full_capture
            .crop(source_region)
            .ok_or(LongShotStartError::InvalidFrame)?;
        self.start(source_region, first_frame, options)
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.source_region = LongShotRect::default();
        self.scroll_seq = 0;
        self.last_appended_scroll_seq = 0;
        self.stitcher = LongScreenshotStitcher::default();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn source_region(&self) -> LongShotRect {
        self.source_region
    }

    pub fn next_scroll_seq(&mut self) -> u64 {
        self.scroll_seq = self.scroll_seq.saturating_add(1);
        self.scroll_seq
    }

    pub fn current_scroll_seq(&self) -> u64 {
        self.scroll_seq
    }

    pub fn can_accept_frame_height(&self, frame_height: i32) -> bool {
        self.active
            && frame_height > 0
            && self.stitcher.height() + frame_height < self.options.max_output_height
    }

    pub fn append_frame(&mut self, frame: LongShotImage) -> LongShotAppendOutcome {
        self.append_frame_for_scroll(self.scroll_seq, frame)
    }

    pub fn append_frame_for_scroll(
        &mut self,
        scroll_seq: u64,
        mut frame: LongShotImage,
    ) -> LongShotAppendOutcome {
        let mut outcome = self.outcome(LongShotAppendStatus::NotActive, scroll_seq);
        if !self.active {
            return outcome;
        }
        if !frame.is_valid_rgba() || frame.height <= 0 {
            outcome.status = LongShotAppendStatus::EmptyFrame;
            return outcome;
        }
        if frame.width != self.source_region.w {
            outcome.status = LongShotAppendStatus::WidthMismatch;
            return outcome;
        }
        if !self.can_accept_frame_height(frame.height) {
            outcome.status = LongShotAppendStatus::TooTall;
            return outcome;
        }
        if scroll_seq != 0 && self.last_appended_scroll_seq == scroll_seq {
            outcome.status = LongShotAppendStatus::SameScrollAlreadyAppended;
            return outcome;
        }

        if self.options.trim_capture_dropout {
            trim_image_trailing_capture_dropout(&mut frame);
        }

        let allow_acceptable_match = scroll_seq == 0 || self.last_appended_scroll_seq != scroll_seq;
        let stitch = self
            .stitcher
            .append(&frame.pixels, frame.height, allow_acceptable_match);
        outcome.stitch = stitch;
        outcome.output_width = self.stitcher.width();
        outcome.output_height = self.stitcher.height();

        if stitch.appended {
            self.last_appended_scroll_seq = scroll_seq;
            outcome.status = LongShotAppendStatus::Appended;
        } else if stitch.duplicate {
            outcome.status = LongShotAppendStatus::Duplicate;
        } else {
            outcome.status = LongShotAppendStatus::Rejected;
        }
        outcome
    }

    pub fn width(&self) -> i32 {
        self.stitcher.width()
    }

    pub fn height(&self) -> i32 {
        self.stitcher.height()
    }

    pub fn pixels(&self) -> &[u8] {
        self.stitcher.pixels()
    }

    pub fn output_image(&self) -> Option<LongShotImage> {
        LongShotImage::from_rgba(self.width(), self.height(), self.pixels().to_vec())
    }

    fn outcome(&self, status: LongShotAppendStatus, scroll_seq: u64) -> LongShotAppendOutcome {
        LongShotAppendOutcome {
            status,
            stitch: LongScreenshotStitchResult::default(),
            scroll_seq,
            output_width: self.stitcher.width(),
            output_height: self.stitcher.height(),
        }
    }
}

pub fn trim_image_trailing_capture_dropout(image: &mut LongShotImage) -> i32 {
    let height = trim_trailing_capture_dropout(&mut image.pixels, image.width, image.height);
    image.height = height;
    height
}

pub fn trim_trailing_capture_dropout(pixels: &mut Vec<u8>, width: i32, height: i32) -> i32 {
    if width <= 0 || height <= 0 || pixels.len() != width as usize * height as usize * 4 {
        return height;
    }

    let is_dropout_row = |y: i32, pixels: &[u8]| {
        let row = y as usize * width as usize * 4;
        let mut near_black = 0;
        let mut dark = 0;
        let mut colored = 0;
        let mut min_luma = 255;
        let mut max_luma = 0;
        for x in 0..width as usize {
            let i = row + x * 4;
            let r = pixels[i] as i32;
            let g = pixels[i + 1] as i32;
            let b = pixels[i + 2] as i32;
            let luma = (r * 299 + g * 587 + b * 114) / 1000;
            if r <= 8 && g <= 8 && b <= 8 {
                near_black += 1;
            }
            if luma <= 42 {
                dark += 1;
            }
            if r > 70 || g > 70 || b > 70 {
                colored += 1;
            }
            min_luma = min_luma.min(luma);
            max_luma = max_luma.max(luma);
        }
        let almost_black = near_black >= width * 95 / 100 && colored <= 1.max(width / 200);
        let uniform_dark = dark >= width * 96 / 100
            && (max_luma - min_luma) <= 18
            && colored <= 1.max(width / 100);
        almost_black || uniform_dark
    };

    let mut dropout_rows = 0;
    let mut y = height - 1;
    while y >= 0 && is_dropout_row(y, pixels) {
        dropout_rows += 1;
        if y == 0 {
            break;
        }
        y -= 1;
    }
    let significant = dropout_rows >= 24 && dropout_rows >= 1.max(height / 12);
    if !significant || dropout_rows >= height - 32 {
        return height;
    }
    let trimmed = height - dropout_rows;
    pixels.resize(width as usize * trimmed as usize * 4, 0);
    trimmed
}
