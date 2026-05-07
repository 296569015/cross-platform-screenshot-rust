use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::thread;

use crate::raster::Image;

use crate::windows_api::core::PCWSTR;
use crate::windows_api::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use crate::windows_api::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, StretchDIBits, UpdateWindow, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, PAINTSTRUCT, SRCCOPY,
};
use crate::windows_api::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::windows_api::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, LoadCursorW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW,
    ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
    GWLP_USERDATA, IDC_ARROW, MSG, SW_SHOW, WM_DESTROY, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
    WM_RBUTTONDOWN, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinWindowOptions {
    pub min_width: i32,
    pub min_height: i32,
    pub topmost: bool,
    pub shadow: bool,
}

impl Default for PinWindowOptions {
    fn default() -> Self {
        Self {
            min_width: 50,
            min_height: 50,
            topmost: true,
            shadow: true,
        }
    }
}

pub fn spawn_pinned_image(image: Image) -> bool {
    spawn_pinned_image_with_options(image, PinWindowOptions::default())
}

pub fn spawn_pinned_image_with_options(image: Image, _options: PinWindowOptions) -> bool {
    if image.width <= 0 || image.height <= 0 || image.pixels.is_empty() {
        return false;
    }

    thread::Builder::new()
        .name("pinned-image-window".to_string())
        .spawn(move || run_pin_window(image))
        .is_ok()
}

struct PinWindow {
    image: Image,
    bgra: Vec<u8>,
}

fn run_pin_window(image: Image) {
    unsafe {
        let hinstance: HINSTANCE = match GetModuleHandleW(PCWSTR::null()) {
            Ok(handle) => handle.into(),
            Err(_) => return,
        };
        let class_name = to_wide("CrossPlatformScreenshotPinnedImage");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(pin_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
            hIconSm: Default::default(),
        };
        RegisterClassExW(&wc);

        let mut state = Box::new(PinWindow {
            bgra: rgba_to_bgra(&image),
            image,
        });
        let ptr = state.as_mut() as *mut PinWindow;
        let title = to_wide("Pinned Screenshot");
        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR::from_raw(class_name.as_ptr()),
            PCWSTR::from_raw(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            state.image.width.max(50),
            state.image.height.max(50),
            None,
            None,
            Some(hinstance),
            Some(ptr.cast::<c_void>() as *const c_void),
        ) {
            Ok(hwnd) => hwnd,
            Err(_) => return,
        };

        let _ = Box::into_raw(state);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn pin_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let createstruct = lparam.0 as *const CREATESTRUCTW;
        let state = (*createstruct).lpCreateParams as *mut PinWindow;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
    }

    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PinWindow;
    match msg {
        WM_PAINT if !state.is_null() => {
            paint_pin_window(hwnd, &*state);
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            if !state.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(state));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint_pin_window(hwnd: HWND, state: &PinWindow) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    if hdc.0.is_null() {
        let _ = EndPaint(hwnd, &ps);
        return;
    }

    let mut client: RECT = zeroed();
    if GetClientRect(hwnd, &mut client).is_ok() {
        let width = state.image.width;
        let height = state.image.height;
        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (width * height * 4) as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let dst_w = client.right - client.left;
        let dst_h = client.bottom - client.top;
        let _ = StretchDIBits(
            hdc,
            0,
            0,
            dst_w,
            dst_h,
            0,
            0,
            width,
            height,
            Some(state.bgra.as_ptr().cast()),
            &bmi,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
    }

    let _ = EndPaint(hwnd, &ps);
}

fn rgba_to_bgra(image: &Image) -> Vec<u8> {
    let mut bgra = vec![0; image.pixels.len()];
    for (src, dst) in image.pixels.chunks_exact(4).zip(bgra.chunks_exact_mut(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = src[3];
    }
    bgra
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
