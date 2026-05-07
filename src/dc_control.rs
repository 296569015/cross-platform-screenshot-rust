use rayon::prelude::*;

use crate::platform::Rect;
use crate::raster::Image;

pub fn multif32toi32(value: f32, dpi: f32) -> i32 {
    (value * dpi).round() as i32
}

pub fn pixelate_region(image: &mut Image, region: Rect, block: i32) {
    let Some(region) = region.intersect(image.bounds()) else {
        return;
    };
    let block = block.max(2);
    let width = image.width as usize;
    for y0 in (region.y..region.bottom()).step_by(block as usize) {
        for x0 in (region.x..region.right()).step_by(block as usize) {
            let x1 = (x0 + block).min(region.right());
            let y1 = (y0 + block).min(region.bottom());
            let mut r = 0_u32;
            let mut g = 0_u32;
            let mut b = 0_u32;
            let mut a = 0_u32;
            let mut count = 0_u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y as usize * width + x as usize) * 4;
                    r += image.pixels[i] as u32;
                    g += image.pixels[i + 1] as u32;
                    b += image.pixels[i + 2] as u32;
                    a += image.pixels[i + 3] as u32;
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let px = [
                (r / count) as u8,
                (g / count) as u8,
                (b / count) as u8,
                (a / count) as u8,
            ];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y as usize * width + x as usize) * 4;
                    image.pixels[i..i + 4].copy_from_slice(&px);
                }
            }
        }
    }
}

pub fn extend_image_center(source: &Image, width: i32, height: i32) -> Image {
    let mut out = Image::new(width.max(1), height.max(1));
    if source.width <= 0 || source.height <= 0 {
        return out;
    }
    let x = (out.width - source.width) / 2;
    let y = (out.height - source.height) / 2;
    out.blit_scaled(
        source,
        Rect {
            x,
            y,
            w: source.width,
            h: source.height,
        },
    );
    out
}

#[cfg(windows)]
pub fn capture_region(desktop_bounds: Rect, region: Rect) -> Option<Image> {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;

    use crate::windows_api::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HGDIOBJ,
        SRCCOPY,
    };

    if region.w <= 0 || region.h <= 0 {
        return None;
    }

    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.0.is_null() {
            return None;
        }
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.0.is_null() {
            ReleaseDC(None, screen_dc);
            return None;
        }

        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: region.w,
            biHeight: -region.h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (region.w * region.h * 4) as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
            .unwrap_or_default();
        if bitmap.0.is_null() || bits.is_null() {
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
            return None;
        }

        let old = SelectObject(mem_dc, HGDIOBJ::from(bitmap));
        let ok = BitBlt(
            mem_dc,
            0,
            0,
            region.w,
            region.h,
            Some(screen_dc),
            desktop_bounds.x + region.x,
            desktop_bounds.y + region.y,
            SRCCOPY | CAPTUREBLT,
        )
        .is_ok();

        let mut rgba = vec![0; region.w as usize * region.h as usize * 4];
        if ok {
            let bgra = std::slice::from_raw_parts(bits as *const u8, rgba.len());
            rgba.par_chunks_exact_mut(4)
                .zip(bgra.par_chunks_exact(4))
                .for_each(|(dst, src)| {
                    dst[0] = src[2];
                    dst[1] = src[1];
                    dst[2] = src[0];
                    dst[3] = 255;
                });
        }

        SelectObject(mem_dc, old);
        let _ = DeleteObject(HGDIOBJ::from(bitmap));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        ok.then_some(Image {
            width: region.w,
            height: region.h,
            pixels: rgba,
        })
    }
}

#[cfg(not(windows))]
pub fn capture_region(_desktop_bounds: Rect, _region: Rect) -> Option<Image> {
    None
}
