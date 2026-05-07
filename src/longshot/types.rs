#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LongShotPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LongShotSize {
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LongShotRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl LongShotRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    pub fn contains(self, point: LongShotPoint) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    pub fn intersect(self, bounds: LongShotRect) -> Option<Self> {
        let x0 = self.x.max(bounds.x);
        let y0 = self.y.max(bounds.y);
        let x1 = self.right().min(bounds.right());
        let y1 = self.bottom().min(bounds.bottom());
        (x1 > x0 && y1 > y0).then_some(Self {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LongShotImage {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl LongShotImage {
    pub fn new(width: i32, height: i32) -> Self {
        let len = rgba_len(width, height).unwrap_or(0);
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }

    pub fn from_rgba(width: i32, height: i32, pixels: Vec<u8>) -> Option<Self> {
        let expected = rgba_len(width, height)?;
        (pixels.len() == expected).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn bounds(&self) -> LongShotRect {
        LongShotRect {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
        }
    }

    pub fn is_valid_rgba(&self) -> bool {
        rgba_len(self.width, self.height).is_some_and(|len| len == self.pixels.len())
    }

    pub fn crop(&self, region: LongShotRect) -> Option<Self> {
        if !self.is_valid_rgba() {
            return None;
        }
        let rect = region.intersect(self.bounds())?;
        let mut out = vec![0; rect.w as usize * rect.h as usize * 4];
        for y in 0..rect.h {
            let src = ((rect.y + y) as usize * self.width as usize + rect.x as usize) * 4;
            let dst = y as usize * rect.w as usize * 4;
            out[dst..dst + rect.w as usize * 4]
                .copy_from_slice(&self.pixels[src..src + rect.w as usize * 4]);
        }
        Self::from_rgba(rect.w, rect.h, out)
    }
}

fn rgba_len(width: i32, height: i32) -> Option<usize> {
    if width <= 0 || height <= 0 {
        return None;
    }
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)
}
