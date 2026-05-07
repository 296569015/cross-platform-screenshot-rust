use std::io::Write;
use std::time::Duration;

use gif::{Encoder, Frame, Repeat};

const DEFAULT_MAX_CACHE_BYTES: usize = 150 * 1024 * 1024;
const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug)]
struct CachedFrame {
    rgba: Vec<u8>,
    delay_cs: u16,
}

#[derive(Clone, Debug)]
pub struct GifTool {
    width: u16,
    height: u16,
    frames: Vec<CachedFrame>,
    cached_bytes: usize,
    max_cache_bytes: usize,
    max_duration_cs: u32,
    duration_cs: u32,
}

impl GifTool {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            frames: Vec::new(),
            cached_bytes: 0,
            max_cache_bytes: DEFAULT_MAX_CACHE_BYTES,
            max_duration_cs: (DEFAULT_MAX_DURATION.as_millis() / 10) as u32,
            duration_cs: 0,
        }
    }

    pub fn with_limits(mut self, max_cache_bytes: usize, max_duration: Duration) -> Self {
        self.max_cache_bytes = max_cache_bytes;
        self.max_duration_cs = (max_duration.as_millis() / 10) as u32;
        self
    }

    pub fn add_frame(&mut self, rgba: &[u8], delay: Duration) -> bool {
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return false;
        }

        let delay_cs = duration_to_centiseconds(delay);
        if self.duration_cs.saturating_add(delay_cs as u32) > self.max_duration_cs {
            return false;
        }

        if let Some(last) = self.frames.last_mut() {
            if last.rgba == rgba {
                last.delay_cs = last.delay_cs.saturating_add(delay_cs).max(1);
                self.duration_cs = self.duration_cs.saturating_add(delay_cs as u32);
                return true;
            }
        }

        if self.cached_bytes.saturating_add(rgba.len()) > self.max_cache_bytes {
            return false;
        }

        self.frames.push(CachedFrame {
            rgba: rgba.to_vec(),
            delay_cs,
        });
        self.cached_bytes += rgba.len();
        self.duration_cs = self.duration_cs.saturating_add(delay_cs as u32);
        true
    }

    pub fn add_emptyframe(&mut self, delay: Duration) -> bool {
        let delay_cs = duration_to_centiseconds(delay);
        if self.frames.is_empty()
            || self.duration_cs.saturating_add(delay_cs as u32) > self.max_duration_cs
        {
            return false;
        }
        if let Some(last) = self.frames.last_mut() {
            last.delay_cs = last.delay_cs.saturating_add(delay_cs).max(1);
        }
        self.duration_cs = self.duration_cs.saturating_add(delay_cs as u32);
        true
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_cs as u64 * 10)
    }

    pub fn encode<W: Write>(&self, writer: W) -> Result<(), gif::EncodingError> {
        let mut encoder = Encoder::new(writer, self.width, self.height, &[])?;
        encoder.set_repeat(Repeat::Infinite)?;

        for cached in &self.frames {
            let mut rgba = cached.rgba.clone();
            let mut frame = Frame::from_rgba_speed(self.width, self.height, &mut rgba, 10);
            frame.delay = cached.delay_cs.max(1);
            encoder.write_frame(&frame)?;
        }

        Ok(())
    }
}

fn duration_to_centiseconds(delay: Duration) -> u16 {
    let cs = (delay.as_millis() / 10).max(1);
    cs.min(u16::MAX as u128) as u16
}
