#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::windows_api::Win32::Foundation::{HWND, LPARAM, POINT, RECT as WinRect, WPARAM};
use crate::windows_api::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, PtInRect,
    ReleaseDC, ScreenToClient, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
    DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use crate::windows_api::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use crate::windows_api::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use crate::windows_api::Win32::UI::WindowsAndMessaging::{
    ChildWindowFromPointEx, GetAncestor, GetTopWindow, GetWindow, GetWindowRect, IsIconic,
    IsWindow, IsWindowVisible, PostMessageW, SetForegroundWindow, WindowFromPoint,
    CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, GA_ROOT, GW_HWNDNEXT,
    PW_RENDERFULLCONTENT, WM_MOUSEWHEEL,
};

use super::{LongShotImage, LongShotPoint, LongShotRect};

trait NullHandle {
    fn is_null(self) -> bool;
}

macro_rules! impl_null_handle {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NullHandle for $ty {
                fn is_null(self) -> bool {
                    self.0.is_null()
                }
            }
        )*
    };
}

impl_null_handle!(HWND, HDC, HBITMAP);

#[derive(Clone, Copy, Debug)]
pub struct LongShotCaptureRequest {
    pub generation: u64,
    pub seq: u64,
    pub desktop_bounds: LongShotRect,
    pub overlay_hwnd: isize,
    pub region: LongShotRect,
}

#[derive(Clone, Debug)]
pub struct LongShotCaptureResponse {
    pub generation: u64,
    pub seq: u64,
    pub frame: Option<LongShotImage>,
}

pub struct LongShotCaptureWorker {
    request_tx: Sender<LongShotCaptureRequest>,
    response_rx: Receiver<LongShotCaptureResponse>,
}

impl LongShotCaptureWorker {
    pub fn spawn() -> Option<Self> {
        let (request_tx, request_rx) = mpsc::channel::<LongShotCaptureRequest>();
        let (response_tx, response_rx) = mpsc::channel::<LongShotCaptureResponse>();
        thread::Builder::new()
            .name("longshot-capture-worker".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let frame = capture_region_or_covered_window(
                        request.desktop_bounds,
                        request.overlay_hwnd,
                        request.region,
                    );
                    if response_tx
                        .send(LongShotCaptureResponse {
                            generation: request.generation,
                            seq: request.seq,
                            frame,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .ok()?;

        Some(Self {
            request_tx,
            response_rx,
        })
    }

    pub fn request(&self, request: LongShotCaptureRequest) -> bool {
        self.request_tx.send(request).is_ok()
    }

    pub fn drain_responses(&self) -> Vec<LongShotCaptureResponse> {
        let mut responses = Vec::new();
        while let Ok(response) = self.response_rx.try_recv() {
            responses.push(response);
        }
        responses
    }

    pub fn wait_for_response(&self, timeout: Duration) -> Option<LongShotCaptureResponse> {
        self.response_rx.recv_timeout(timeout).ok()
    }
}

pub fn capture_region_or_covered_window(
    desktop_bounds: LongShotRect,
    overlay_hwnd: isize,
    region: LongShotRect,
) -> Option<LongShotImage> {
    capture_region(desktop_bounds, region)
        .or_else(|| capture_covered_window(desktop_bounds, hwnd_from_raw(overlay_hwnd), region))
}

pub fn capture_region(desktop_bounds: LongShotRect, region: LongShotRect) -> Option<LongShotImage> {
    if region.is_empty() {
        return None;
    }
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_null() {
            return None;
        }
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_null() {
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
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap: HBITMAP =
            CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .unwrap_or_default();
        if bitmap.is_null() || bits.is_null() {
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
            for (src, dst) in bgra.chunks_exact(4).zip(rgba.chunks_exact_mut(4)) {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = 255;
            }
        }
        SelectObject(mem_dc, old);
        let _ = DeleteObject(HGDIOBJ::from(bitmap));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
        ok.then_some(LongShotImage {
            width: region.w,
            height: region.h,
            pixels: rgba,
        })
    }
}

pub fn post_mouse_wheel_at(
    screen_point: LongShotPoint,
    wheel_delta: i32,
    overlay_hwnd: isize,
    last_scroll_target: &mut isize,
) -> bool {
    unsafe {
        let point = POINT {
            x: screen_point.x,
            y: screen_point.y,
        };
        let overlay = hwnd_from_raw(overlay_hwnd);
        let mut target = WindowFromPoint(point);
        if !overlay.is_null()
            && !target.is_null()
            && (target == overlay || GetAncestor(target, GA_ROOT) == overlay)
        {
            target = find_window_from_point_excluding(point, overlay);
        }
        if target.is_null() {
            let previous = hwnd_from_raw(*last_scroll_target);
            if !previous.is_null() && IsWindow(Some(previous)).0 != 0 {
                target = previous;
            } else {
                return false;
            }
        }

        *last_scroll_target = hwnd_to_raw(target);
        let root = GetAncestor(target, GA_ROOT);
        if !root.is_null() {
            let _ = SetForegroundWindow(root);
        }

        let wparam = WPARAM((wheel_delta as i16 as u16 as usize) << 16);
        PostMessageW(
            Some(target),
            WM_MOUSEWHEEL,
            wparam,
            make_lparam(screen_point.x, screen_point.y),
        )
        .is_ok()
    }
}

fn capture_covered_window(
    desktop_bounds: LongShotRect,
    overlay_hwnd: HWND,
    region: LongShotRect,
) -> Option<LongShotImage> {
    if region.is_empty() {
        return None;
    }
    unsafe {
        let screen_point = POINT {
            x: desktop_bounds.x + region.x + region.w / 2,
            y: desktop_bounds.y + region.y + region.h / 2,
        };
        let mut target = WindowFromPoint(screen_point);
        if !overlay_hwnd.is_null()
            && !target.is_null()
            && (target == overlay_hwnd || GetAncestor(target, GA_ROOT) == overlay_hwnd)
        {
            target = find_window_from_point_excluding(screen_point, overlay_hwnd);
        }
        if target.is_null()
            || (!overlay_hwnd.is_null() && GetAncestor(target, GA_ROOT) == overlay_hwnd)
        {
            return None;
        }

        let root = GetAncestor(target, GA_ROOT);
        if root.is_null() || IsWindowVisible(root).0 == 0 || IsIconic(root).0 != 0 {
            return None;
        }

        let mut window_rect: WinRect = zeroed();
        if GetWindowRect(root, &mut window_rect).is_err() {
            return None;
        }
        let window_w = window_rect.right - window_rect.left;
        let window_h = window_rect.bottom - window_rect.top;
        if window_w <= 0 || window_h <= 0 {
            return None;
        }

        let capture_rect = WinRect {
            left: desktop_bounds.x + region.x,
            top: desktop_bounds.y + region.y,
            right: desktop_bounds.x + region.x + region.w,
            bottom: desktop_bounds.y + region.y + region.h,
        };
        if capture_rect.left < window_rect.left
            || capture_rect.top < window_rect.top
            || capture_rect.right > window_rect.right
            || capture_rect.bottom > window_rect.bottom
        {
            return None;
        }

        let screen_dc = GetDC(None);
        if screen_dc.is_null() {
            return None;
        }
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_null() {
            ReleaseDC(None, screen_dc);
            return None;
        }

        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: window_w,
            biHeight: -window_h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (window_w * window_h * 4) as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap: HBITMAP =
            CreateDIBSection(Some(screen_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .unwrap_or_default();
        if bitmap.is_null() || bits.is_null() {
            if !bitmap.is_null() {
                let _ = DeleteObject(HGDIOBJ::from(bitmap));
            }
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
            return None;
        }

        let old = SelectObject(mem_dc, HGDIOBJ::from(bitmap));
        let printed = PrintWindow(root, mem_dc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT));
        let mut image = None;
        if printed.0 != 0 {
            let mut rgba = vec![0; region.w as usize * region.h as usize * 4];
            let bgra = std::slice::from_raw_parts(
                bits as *const u8,
                window_w as usize * window_h as usize * 4,
            );
            let src_x = capture_rect.left - window_rect.left;
            let src_y = capture_rect.top - window_rect.top;
            for y in 0..region.h as usize {
                let src = ((src_y as usize + y) * window_w as usize + src_x as usize) * 4;
                let dst = y * region.w as usize * 4;
                for x in 0..region.w as usize {
                    let si = src + x * 4;
                    let di = dst + x * 4;
                    rgba[di] = bgra[si + 2];
                    rgba[di + 1] = bgra[si + 1];
                    rgba[di + 2] = bgra[si];
                    rgba[di + 3] = 255;
                }
            }
            image = Some(LongShotImage {
                width: region.w,
                height: region.h,
                pixels: rgba,
            });
        }

        SelectObject(mem_dc, old);
        let _ = DeleteObject(HGDIOBJ::from(bitmap));
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
        image
    }
}

unsafe fn find_window_from_point_excluding(point: POINT, ignored: HWND) -> HWND {
    let mut hwnd = GetTopWindow(None).unwrap_or_default();
    while !hwnd.is_null() {
        if hwnd != ignored
            && GetAncestor(hwnd, GA_ROOT) != ignored
            && IsWindowVisible(hwnd).0 != 0
            && IsWindowEnabled(hwnd).0 != 0
        {
            let mut rect: WinRect = zeroed();
            if GetWindowRect(hwnd, &mut rect).is_ok() && PtInRect(&rect, point).0 != 0 {
                let mut client = point;
                let _ = ScreenToClient(hwnd, &mut client);
                let child = ChildWindowFromPointEx(
                    hwnd,
                    client,
                    CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
                );
                return if child.is_null() { hwnd } else { child };
            }
        }
        hwnd = GetWindow(hwnd, GW_HWNDNEXT).unwrap_or_default();
    }
    HWND::default()
}

fn hwnd_from_raw(raw: isize) -> HWND {
    HWND(raw as *mut c_void)
}

fn hwnd_to_raw(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn make_lparam(x: i32, y: i32) -> LPARAM {
    LPARAM((((y as u16 as u32) << 16) | (x as u16 as u32)) as isize)
}
