use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::platform::Color;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub linewidth: f32,
    pub fontsize: f32,
    pub color: ConfigColor,
    pub mosaicstyle: MosaicStyle,
    pub gif_fps: u32,
    pub gif_max_seconds: u32,
    pub gif_max_cache_mb: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ConfigColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MosaicStyle {
    Pixelate,
    Blur,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            linewidth: 2.0,
            fontsize: 18.0,
            color: ConfigColor {
                r: 255,
                g: 92,
                b: 92,
                a: 255,
            },
            mosaicstyle: MosaicStyle::Pixelate,
            gif_fps: 10,
            gif_max_seconds: 300,
            gif_max_cache_mb: 150,
        }
    }
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text)
    }

    pub fn draw_color(&self) -> Color {
        Color::rgba(self.color.r, self.color.g, self.color.b, self.color.a)
    }
}
