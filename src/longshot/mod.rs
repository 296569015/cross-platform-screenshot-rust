//! Standalone long-screenshot package.
//!
//! Copy this folder into another Rust screenshot tool when you want the
//! long-screenshot capability without dragging in this demo application's UI.

mod session;
mod stitcher;
mod types;

pub use session::{
    trim_image_trailing_capture_dropout, trim_trailing_capture_dropout, LongShotAppendOutcome,
    LongShotAppendStatus, LongShotSession, LongShotSessionOptions, LongShotStartError,
    DEFAULT_LONG_SHOT_MAX_OUTPUT_HEIGHT,
};
pub use stitcher::{
    LongScreenshotStitchOptions, LongScreenshotStitchResult, LongScreenshotStitcher,
};
pub use types::{LongShotImage, LongShotPoint, LongShotRect, LongShotSize};

#[cfg(windows)]
pub mod windows;
