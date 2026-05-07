use crate::core::{
    Annotation, ArrowAnnotation, FreehandAnnotation, LineAnnotation, RectAnnotation,
};
use crate::platform::{Color, PointF, Rect};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: i32, height: i32) -> Self {
        let len = width.max(0) as usize * height.max(0) as usize * 4;
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }

    pub fn from_rgba(width: i32, height: i32, pixels: Vec<u8>) -> Option<Self> {
        let expected = width.max(0) as usize * height.max(0) as usize * 4;
        (width > 0 && height > 0 && pixels.len() == expected).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn bounds(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
        }
    }

    pub fn crop(&self, region: Rect) -> Option<Self> {
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

    pub fn blit_scaled(&mut self, source: &Image, dst: Rect) {
        if source.width <= 0 || source.height <= 0 || dst.w <= 0 || dst.h <= 0 {
            return;
        }
        let clipped = match dst.intersect(self.bounds()) {
            Some(rect) => rect,
            None => return,
        };
        for y in clipped.y..clipped.bottom() {
            for x in clipped.x..clipped.right() {
                let u = ((x - dst.x) as f32 / dst.w as f32 * source.width as f32)
                    .floor()
                    .clamp(0.0, (source.width - 1) as f32) as i32;
                let v = ((y - dst.y) as f32 / dst.h as f32 * source.height as f32)
                    .floor()
                    .clamp(0.0, (source.height - 1) as f32) as i32;
                let src = (v as usize * source.width as usize + u as usize) * 4;
                let dst_i = (y as usize * self.width as usize + x as usize) * 4;
                self.pixels[dst_i..dst_i + 4].copy_from_slice(&source.pixels[src..src + 4]);
            }
        }
    }

    pub fn blend_rect(&mut self, rect: Rect, color: Color) {
        let rect = match rect.intersect(self.bounds()) {
            Some(rect) => rect,
            None => return,
        };
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                self.blend_pixel(x, y, color);
            }
        }
    }

    pub fn fill_round_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        let clipped = match rect.intersect(self.bounds()) {
            Some(rect) => rect,
            None => return,
        };
        let r = radius.max(0.0).min(rect.w.min(rect.h) as f32 * 0.5);
        for y in clipped.y..clipped.bottom() {
            for x in clipped.x..clipped.right() {
                if point_in_round_rect(x as f32 + 0.5, y as f32 + 0.5, rect, r) {
                    self.blend_pixel(x, y, color);
                }
            }
        }
    }

    pub fn draw_rect_outline(&mut self, rect: Rect, color: Color, thickness: f32) {
        let t = thickness.max(1.0).round() as i32;
        self.blend_rect(
            Rect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: t,
            },
            color,
        );
        self.blend_rect(
            Rect {
                x: rect.x,
                y: rect.bottom() - t,
                w: rect.w,
                h: t,
            },
            color,
        );
        self.blend_rect(
            Rect {
                x: rect.x,
                y: rect.y,
                w: t,
                h: rect.h,
            },
            color,
        );
        self.blend_rect(
            Rect {
                x: rect.right() - t,
                y: rect.y,
                w: t,
                h: rect.h,
            },
            color,
        );
    }

    pub fn draw_ellipse_outline(&mut self, rect: Rect, color: Color, thickness: f32) {
        if rect.w <= 1 || rect.h <= 1 {
            return;
        }
        let clipped = match rect.intersect(self.bounds()) {
            Some(rect) => rect,
            None => return,
        };
        let rx = rect.w as f32 * 0.5;
        let ry = rect.h as f32 * 0.5;
        if rx <= 0.5 || ry <= 0.5 {
            return;
        }
        let cx = rect.x as f32 + rx;
        let cy = rect.y as f32 + ry;
        let t = thickness.max(1.0);
        let inner_rx = (rx - t).max(0.0);
        let inner_ry = (ry - t).max(0.0);

        for y in clipped.y..clipped.bottom() {
            for x in clipped.x..clipped.right() {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let outer = dx * dx + dy * dy;
                if outer > 1.0 {
                    continue;
                }

                if inner_rx <= 0.0 || inner_ry <= 0.0 {
                    self.blend_pixel(x, y, color);
                    continue;
                }

                let idx = (x as f32 + 0.5 - cx) / inner_rx;
                let idy = (y as f32 + 0.5 - cy) / inner_ry;
                if idx * idx + idy * idy >= 1.0 {
                    self.blend_pixel(x, y, color);
                }
            }
        }
    }

    pub fn draw_line(&mut self, start: PointF, end: PointF, color: Color, thickness: f32) {
        let radius = (thickness.max(1.0) * 0.5).max(0.5);
        let min_x = start.x.min(end.x).floor() as i32 - radius.ceil() as i32 - 1;
        let max_x = start.x.max(end.x).ceil() as i32 + radius.ceil() as i32 + 1;
        let min_y = start.y.min(end.y).floor() as i32 - radius.ceil() as i32 - 1;
        let max_y = start.y.max(end.y).ceil() as i32 + radius.ceil() as i32 + 1;
        let bounds = Rect {
            x: min_x,
            y: min_y,
            w: max_x - min_x + 1,
            h: max_y - min_y + 1,
        };
        let bounds = match bounds.intersect(self.bounds()) {
            Some(bounds) => bounds,
            None => return,
        };
        for y in bounds.y..bounds.bottom() {
            for x in bounds.x..bounds.right() {
                let p = PointF {
                    x: x as f32 + 0.5,
                    y: y as f32 + 0.5,
                };
                let d = distance_to_segment(p, start, end);
                if d <= radius {
                    let mut c = color;
                    let coverage = (radius + 0.75 - d).clamp(0.0, 1.0);
                    c.a = ((c.a as f32) * coverage) as u8;
                    self.blend_pixel(x, y, c);
                }
            }
        }
    }

    pub fn draw_arrow(&mut self, arrow: &ArrowAnnotation) {
        self.draw_line(arrow.start, arrow.end, arrow.color, arrow.thickness);
        let dx = arrow.end.x - arrow.start.x;
        let dy = arrow.end.y - arrow.start.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }
        let ux = dx / len;
        let uy = dy / len;
        let px = -uy;
        let py = ux;
        let base = PointF {
            x: arrow.end.x - ux * arrow.head_size,
            y: arrow.end.y - uy * arrow.head_size,
        };
        let left = PointF {
            x: base.x + px * arrow.head_size * 0.48,
            y: base.y + py * arrow.head_size * 0.48,
        };
        let right = PointF {
            x: base.x - px * arrow.head_size * 0.48,
            y: base.y - py * arrow.head_size * 0.48,
        };
        self.draw_line(arrow.end, left, arrow.color, arrow.thickness);
        self.draw_line(arrow.end, right, arrow.color, arrow.thickness);
    }

    pub fn draw_polyline(&mut self, points: &[PointF], color: Color, thickness: f32) {
        for pair in points.windows(2) {
            self.draw_line(pair[0], pair[1], color, thickness);
        }
    }

    pub fn draw_annotation(&mut self, annotation: &Annotation, origin_x: f32, origin_y: f32) {
        match annotation {
            Annotation::Rect(RectAnnotation {
                bounds,
                color,
                thickness,
                filled,
            }) => {
                let rect = Rect {
                    x: (bounds.x - origin_x).round() as i32,
                    y: (bounds.y - origin_y).round() as i32,
                    w: bounds.w.round() as i32,
                    h: bounds.h.round() as i32,
                };
                if *filled {
                    self.blend_rect(rect, *color);
                } else {
                    self.draw_rect_outline(rect, *color, *thickness);
                }
            }
            Annotation::Arrow(arrow) => {
                let mut arrow = arrow.clone();
                arrow.start.x -= origin_x;
                arrow.start.y -= origin_y;
                arrow.end.x -= origin_x;
                arrow.end.y -= origin_y;
                self.draw_arrow(&arrow);
            }
            Annotation::Line(LineAnnotation {
                start,
                end,
                color,
                thickness,
            }) => {
                self.draw_line(
                    PointF {
                        x: start.x - origin_x,
                        y: start.y - origin_y,
                    },
                    PointF {
                        x: end.x - origin_x,
                        y: end.y - origin_y,
                    },
                    *color,
                    *thickness,
                );
            }
            Annotation::Freehand(FreehandAnnotation {
                points,
                color,
                thickness,
            }) => {
                let shifted: Vec<_> = points
                    .iter()
                    .map(|p| PointF {
                        x: p.x - origin_x,
                        y: p.y - origin_y,
                    })
                    .collect();
                self.draw_polyline(&shifted, *color, *thickness);
            }
            Annotation::Text(_) => {}
        }
    }

    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height || color.a == 0 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let a = color.a as u32;
        let inv = 255_u32.saturating_sub(a);
        self.pixels[i] = ((color.r as u32 * a + self.pixels[i] as u32 * inv) / 255) as u8;
        self.pixels[i + 1] = ((color.g as u32 * a + self.pixels[i + 1] as u32 * inv) / 255) as u8;
        self.pixels[i + 2] = ((color.b as u32 * a + self.pixels[i + 2] as u32 * inv) / 255) as u8;
        self.pixels[i + 3] = 255;
    }
}

pub fn smooth_freehand_points(points: &[PointF]) -> Vec<PointF> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut smoothed = Vec::with_capacity(points.len());
    smoothed.push(points[0]);
    for triple in points.windows(3) {
        smoothed.push(PointF {
            x: (triple[0].x + triple[1].x * 2.0 + triple[2].x) * 0.25,
            y: (triple[0].y + triple[1].y * 2.0 + triple[2].y) * 0.25,
        });
    }
    smoothed.push(*points.last().unwrap());
    smoothed
}

pub fn append_freehand_point(points: &mut Vec<PointF>, x: f32, y: f32) {
    let point = PointF { x, y };
    if points
        .last()
        .is_none_or(|last| ((last.x - x).powi(2) + (last.y - y).powi(2)).sqrt() >= 1.5)
    {
        points.push(point);
    }
}

fn point_in_round_rect(px: f32, py: f32, rect: Rect, radius: f32) -> bool {
    if radius <= 0.5 {
        return true;
    }
    let x = rect.x as f32;
    let y = rect.y as f32;
    let w = rect.w as f32;
    let h = rect.h as f32;
    let cx = px.clamp(x + radius, x + w - radius);
    let cy = py.clamp(y + radius, y + h - radius);
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= radius * radius
}

fn distance_to_segment(p: PointF, a: PointF, b: PointF) -> f32 {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let wx = p.x - a.x;
    let wy = p.y - a.y;
    let len2 = vx * vx + vy * vy;
    if len2 <= f32::EPSILON {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    let t = ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0);
    let proj = PointF {
        x: a.x + t * vx,
        y: a.y + t * vy,
    };
    ((p.x - proj.x).powi(2) + (p.y - proj.y).powi(2)).sqrt()
}
