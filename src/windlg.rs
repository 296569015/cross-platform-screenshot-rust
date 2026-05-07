#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_void, OsStr, OsString};
use std::io::Write;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::windows_api::core::{PCWSTR, PWSTR};
use crate::windows_api::Win32::Foundation::{
    HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT as WinRect, WPARAM,
};
use crate::windows_api::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CombineRgn, CreateCompatibleDC, CreateDIBSection, CreateRectRgn, DeleteDC,
    DeleteObject, EndPaint, GetDC, InvalidateRect, PtInRect, ReleaseDC, ScreenToClient,
    SelectObject, SetDIBitsToDevice, SetWindowRgn, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CAPTUREBLT, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, HRGN, PAINTSTRUCT, RGN_OR, SRCCOPY,
};
use crate::windows_api::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use crate::windows_api::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use crate::windows_api::Win32::System::LibraryLoader::GetModuleHandleW;
use crate::windows_api::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
};
use crate::windows_api::Win32::UI::Controls::Dialogs::{
    GetSaveFileNameW, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use crate::windows_api::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use crate::windows_api::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, GetKeyState, IsWindowEnabled, RegisterHotKey, ReleaseCapture, SetCapture, SetFocus,
    UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, VK_ESCAPE,
};
use crate::windows_api::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use crate::windows_api::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, ChildWindowFromPointEx, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, GetAncestor, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetTopWindow, GetWindow, GetWindowLongPtrW, GetWindowRect,
    IsIconic, IsWindow, IsWindowVisible, KillTimer, LoadCursorW, LoadIconW, PostMessageW,
    PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowPos, SetWindowsHookExW, ShowWindow, TrackPopupMenu, TranslateMessage,
    UnhookWindowsHookEx, WindowFromPoint, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CWP_SKIPDISABLED,
    CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, GA_ROOT, GWLP_USERDATA, GW_HWNDNEXT, HHOOK, HMENU,
    IDC_CROSS, IDI_APPLICATION, MF_SEPARATOR, MF_STRING, MSG, MSLLHOOKSTRUCT, PW_RENDERFULLCONTENT,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, TPM_RIGHTBUTTON, WH_MOUSE_LL, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCCREATE, WM_PAINT, WM_RBUTTONDOWN, WM_TIMER, WM_USER, WNDCLASSEXW,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::core::{
    Annotation, AnnotationModel, AnnotationTool, AppEvent, AppState, ArrowAnnotation,
    CommandHistory, FreehandAnnotation, LineAnnotation, LongScreenshotStitcher, RectAnnotation,
    ScreenshotStateMachine,
};
use crate::longshot::{trim_trailing_capture_dropout, LongShotSessionOptions};
use crate::platform::{Color, Point, PointF, Rect, RectF, Size};
use crate::png::encode_rgba_png;
use crate::raster::{append_freehand_point, smooth_freehand_points, Image};

const CLASS_NAME: &str = "CrossPlatformScreenshotRustOverlay";
const WM_TRAYICON: u32 = WM_USER + 1;
const WM_LONG_WHEEL: u32 = WM_USER + 2;
const TIMER_ID: usize = 1;
const HOTKEY_CTRL_ALT_X: i32 = 100;
const HOTKEY_F8: i32 = 101;
const HOTKEY_CTRL_SHIFT_A: i32 = 102;
const TRAY_ID: u32 = 1;
const TRAY_CAPTURE_ID: usize = 2001;
const TRAY_QUIT_ID: usize = 2002;
const CF_DIB_FORMAT: u32 = 8;

const BTN_SIZE: f32 = 48.0;
const BTN_GAP: f32 = 26.0;
const TOOLBAR_PAD: f32 = 18.0;
const TOOLBAR_GAP: f32 = 12.0;
const TOOLBAR_GROUP_GAP: f32 = 12.0;
const CORNER_R: f32 = 9.0;
const BTN_CORNER_R: f32 = 6.0;
const LONG_TOOLBAR_BTN_SIZE: f32 = 40.0;
const STYLE_PANEL_GAP: f32 = 22.0;
const STYLE_PANEL_PAD: f32 = 12.0;
const STYLE_PANEL_LABEL_W: f32 = 98.0;
const STYLE_PANEL_CONTROLS_W: f32 = 1048.0;
const STYLE_PANEL_H: f32 = 78.0;
const STYLE_PANEL_SECTION_GAP: f32 = 20.0;
const LONG_CAPTURE_DELAY: Duration = Duration::from_millis(50);
const LONG_NATIVE_CAPTURE_DELAY: Duration = Duration::from_millis(28);
const LONG_TRAILING_CAPTURE_DELAY: Duration = Duration::from_millis(200);
const LONG_MIN_CAPTURE_INTERVAL: Duration = Duration::from_millis(35);
const LONG_AUTO_SCROLL_INTERVAL: Duration = Duration::from_millis(120);
const LONG_AUTO_CAPTURE_DELAY: Duration = Duration::from_millis(45);
const LONG_AUTO_CAPTURE_INTERVAL: Duration = Duration::from_millis(55);
const LONG_AUTO_PREVIEW_RENDER_INTERVAL: Duration = Duration::from_millis(360);
const LONG_AUTO_SCROLL_DELTA: f32 = -0.33;
const LONG_MAX_OUTPUT_HEIGHT: i32 = 16_000;
const ENABLE_LONG_CAPTURE_LOGS: bool = cfg!(debug_assertions);

static HOOK_HWND: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

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

impl_null_handle!(HANDLE, HGLOBAL, HWND, HBITMAP, HDC, HGDIOBJ, HRGN, HHOOK, HMENU);

fn pcwstr(value: &[u16]) -> PCWSTR {
    PCWSTR::from_raw(value.as_ptr())
}

fn pwstr(value: &mut [u16]) -> PWSTR {
    PWSTR::from_raw(value.as_mut_ptr())
}

const TOOLBAR_BG: Color = Color::rgba(255, 255, 255, 246);
const TOOLBAR_STROKE: Color = Color::rgba(210, 214, 220, 190);
const TOOLBAR_SHADOW: Color = Color::rgba(0, 0, 0, 46);
const BTN_HOVER: Color = Color::rgba(232, 234, 238, 255);
const BTN_DOWN: Color = Color::rgba(225, 241, 255, 255);
const BTN_ICON: Color = Color::rgba(17, 17, 17, 255);
const BTN_ICON_DOWN: Color = Color::rgba(24, 126, 235, 255);
const SEL_BORDER: Color = Color::rgba(24, 126, 235, 255);
const SEL_ACCENT: Color = Color::rgba(24, 126, 235, 255);
const BADGE_TEXT: Color = Color::rgba(255, 255, 255, 255);
const ANNOTATION_RED: Color = Color::rgba(255, 92, 92, 255);
const ACTION_SUCCESS: Color = Color::rgba(24, 126, 235, 255);
const DIM_COLOR: Color = Color::rgba(4, 8, 12, 168);
const LONG_DIM_COLOR: Color = Color::rgba(4, 8, 12, 178);
const LONG_PANEL: Color = Color::rgba(247, 250, 252, 245);
const LONG_TOOLBAR: Color = TOOLBAR_BG;
const LONG_BORDER: Color = Color::rgba(255, 92, 92, 255);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolButtonType {
    Rectangle,
    Circle,
    AddText,
    NumberMarker,
    Pen,
    Arrow,
    Line,
    DashedLine,
    Mosaic,
    Gif,
    Pin,
    Ocr,
    LongScreenshot,
    Edit,
    AutoScroll,
    Undo,
    Save,
    Cancel,
    Confirm,
}

#[derive(Clone, Copy, Debug)]
struct ToolButton {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    button_type: ToolButtonType,
    hovered: bool,
}

struct LongCaptureWorker {
    request_tx: Sender<LongCaptureRequest>,
    response_rx: Receiver<LongCaptureResponse>,
}

#[derive(Clone, Copy)]
struct LongCaptureRequest {
    generation: u64,
    seq: u64,
    desktop_bounds: Rect,
    overlay_hwnd: isize,
    region: Rect,
}

struct LongCaptureResponse {
    generation: u64,
    seq: u64,
    frame: Option<Image>,
}

impl LongCaptureWorker {
    fn spawn() -> Option<Self> {
        let (request_tx, request_rx) = mpsc::channel::<LongCaptureRequest>();
        let (response_tx, response_rx) = mpsc::channel::<LongCaptureResponse>();
        thread::Builder::new()
            .name("long-capture-worker".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let frame =
                        capture_region(request.desktop_bounds, request.region).or_else(|| {
                            let overlay_hwnd = HWND(request.overlay_hwnd as *mut c_void);
                            capture_covered_window(
                                request.desktop_bounds,
                                overlay_hwnd,
                                request.region,
                            )
                        });
                    if response_tx
                        .send(LongCaptureResponse {
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

    fn request(&self, request: LongCaptureRequest) -> bool {
        self.request_tx.send(request).is_ok()
    }

    fn drain_responses(&self) -> Vec<LongCaptureResponse> {
        let mut responses = Vec::new();
        while let Ok(response) = self.response_rx.try_recv() {
            responses.push(response);
        }
        responses
    }

    fn wait_for_response(&self, timeout: Duration) -> Option<LongCaptureResponse> {
        self.response_rx.recv_timeout(timeout).ok()
    }
}

pub fn run() -> i32 {
    install_panic_logger();
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let mut app = match Application::create() {
        Ok(app) => app,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    app.message_loop()
}

struct Application {
    hwnd: HWND,
    bounds: Rect,
    screen_size: Size,
    state_machine: ScreenshotStateMachine,
    annotations: AnnotationModel,
    command_history: CommandHistory,
    captured: Option<Image>,
    long_background: Option<Image>,
    is_long_result: bool,
    is_long_capture_active: bool,
    long_auto_scroll_active: bool,
    long_auto_stalls: i32,
    next_long_auto_scroll: Instant,
    next_long_auto_preview_render: Instant,
    pending_long_frame_capture: bool,
    long_frame_capture_due: Instant,
    long_trailing_capture_due: Instant,
    long_needs_trailing_capture: bool,
    long_scroll_seq: u64,
    long_pending_scroll_seq: u64,
    long_pending_scroll_at: Instant,
    long_current_scroll_seq: u64,
    long_current_scroll_at: Instant,
    long_last_appended_scroll_seq: u64,
    long_last_frame_capture: Instant,
    long_source_region: Rect,
    long_stitcher: LongScreenshotStitcher,
    long_capture_worker: Option<LongCaptureWorker>,
    long_capture_generation: u64,
    long_capture_in_flight: bool,
    long_capture_in_flight_seq: u64,
    long_capture_in_flight_needs_trailing: bool,
    long_capture_in_flight_trailing_due: Instant,
    long_preview_thumbnail: Option<Image>,
    long_preview_thumbnail_rect: Rect,
    last_scroll_target: HWND,
    mouse_hook: HHOOK,
    passthrough_region: Option<Rect>,
    overlay_regions: Vec<Rect>,
    is_dragging: bool,
    drag_start: PointF,
    drag_current: PointF,
    is_drawing_annotation: bool,
    annotation_start: PointF,
    annotation_current: PointF,
    active_freehand_points: Vec<PointF>,
    active_tool: AnnotationTool,
    annotation_color: Color,
    annotation_thickness: f32,
    tool_buttons: Vec<ToolButton>,
    pressed_toolbar_button: Option<ToolButtonType>,
    toolbar_x: f32,
    toolbar_y: f32,
    toolbar_w: f32,
    toolbar_h: f32,
    style_panel_x: f32,
    style_panel_y: f32,
    style_panel_w: f32,
    style_panel_h: f32,
    tray_menu: HMENU,
    tray_data: NOTIFYICONDATAW,
}

impl Application {
    fn create() -> Result<Box<Self>, String> {
        unsafe {
            let hinstance: HINSTANCE = GetModuleHandleW(PCWSTR::null())
                .map_err(|error| format!("failed to get module handle: {error}"))?
                .into();
            let class_name = to_wide(CLASS_NAME);
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
                hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
                hbrBackground: Default::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: pcwstr(&class_name),
                hIconSm: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            };
            RegisterClassExW(&wc);

            let bounds = virtual_desktop_bounds();
            let screen_size = Size {
                w: bounds.w,
                h: bounds.h,
            };
            let now = Instant::now();

            let mut app = Box::new(Self {
                hwnd: HWND::default(),
                bounds,
                screen_size,
                state_machine: ScreenshotStateMachine::default(),
                annotations: AnnotationModel::default(),
                command_history: CommandHistory::default(),
                captured: None,
                long_background: None,
                is_long_result: false,
                is_long_capture_active: false,
                long_auto_scroll_active: false,
                long_auto_stalls: 0,
                next_long_auto_scroll: now,
                next_long_auto_preview_render: now,
                pending_long_frame_capture: false,
                long_frame_capture_due: now,
                long_trailing_capture_due: now,
                long_needs_trailing_capture: false,
                long_scroll_seq: 0,
                long_pending_scroll_seq: 0,
                long_pending_scroll_at: now,
                long_current_scroll_seq: 0,
                long_current_scroll_at: now,
                long_last_appended_scroll_seq: 0,
                long_last_frame_capture: now,
                long_source_region: Rect::default(),
                long_stitcher: LongScreenshotStitcher::default(),
                long_capture_worker: LongCaptureWorker::spawn(),
                long_capture_generation: 0,
                long_capture_in_flight: false,
                long_capture_in_flight_seq: 0,
                long_capture_in_flight_needs_trailing: false,
                long_capture_in_flight_trailing_due: now,
                long_preview_thumbnail: None,
                long_preview_thumbnail_rect: Rect::default(),
                last_scroll_target: HWND::default(),
                mouse_hook: HHOOK::default(),
                passthrough_region: None,
                overlay_regions: Vec::new(),
                is_dragging: false,
                drag_start: PointF::default(),
                drag_current: PointF::default(),
                is_drawing_annotation: false,
                annotation_start: PointF::default(),
                annotation_current: PointF::default(),
                active_freehand_points: Vec::new(),
                active_tool: AnnotationTool::Rectangle,
                annotation_color: ANNOTATION_RED,
                annotation_thickness: 2.0,
                tool_buttons: Vec::new(),
                pressed_toolbar_button: None,
                toolbar_x: 0.0,
                toolbar_y: 0.0,
                toolbar_w: 0.0,
                toolbar_h: 0.0,
                style_panel_x: 0.0,
                style_panel_y: 0.0,
                style_panel_w: 0.0,
                style_panel_h: 0.0,
                tray_menu: HMENU::default(),
                tray_data: zeroed(),
            });

            let app_ptr = app.as_mut() as *mut Self;
            let title = to_wide("Screenshot");
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                pcwstr(&class_name),
                pcwstr(&title),
                WS_POPUP,
                bounds.x,
                bounds.y,
                bounds.w,
                bounds.h,
                None,
                None,
                Some(hinstance),
                Some(app_ptr.cast::<c_void>() as *const c_void),
            )
            .map_err(|error| format!("failed to create overlay window: {error}"))?;
            app.hwnd = hwnd;
            HOOK_HWND.store(hwnd.0, Ordering::SeqCst);
            app.register_hotkeys();
            app.create_tray();
            SetTimer(Some(hwnd), TIMER_ID, 30, None);
            Ok(app)
        }
    }

    fn message_loop(&mut self) -> i32 {
        unsafe {
            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        0
    }

    unsafe fn register_hotkeys(&self) {
        let ctrl_alt = MOD_CONTROL | MOD_ALT | MOD_NOREPEAT;
        let ctrl_shift = MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT;
        let _ = RegisterHotKey(Some(self.hwnd), HOTKEY_CTRL_ALT_X, ctrl_alt, 'X' as u32);
        let _ = RegisterHotKey(Some(self.hwnd), HOTKEY_F8, MOD_NOREPEAT, 0x77);
        let _ = RegisterHotKey(Some(self.hwnd), HOTKEY_CTRL_SHIFT_A, ctrl_shift, 'A' as u32);
    }

    unsafe fn unregister_hotkeys(&self) {
        let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_CTRL_ALT_X);
        let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_F8);
        let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_CTRL_SHIFT_A);
    }

    unsafe fn create_tray(&mut self) {
        let Ok(tray_menu) = CreatePopupMenu() else {
            append_log("failed to create tray menu");
            return;
        };
        self.tray_menu = tray_menu;
        let capture = to_wide("Take Screenshot");
        let _ = AppendMenuW(self.tray_menu, MF_STRING, TRAY_CAPTURE_ID, pcwstr(&capture));
        let _ = AppendMenuW(self.tray_menu, MF_SEPARATOR, 0, PCWSTR::null());
        let quit = to_wide("Quit");
        let _ = AppendMenuW(self.tray_menu, MF_STRING, TRAY_QUIT_ID, pcwstr(&quit));

        let mut nid: NOTIFYICONDATAW = zeroed();
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = TRAY_ID;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        let tip = to_wide("Screenshot Tool");
        copy_wide_into(&mut nid.szTip, &tip);
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        self.tray_data = nid;
    }

    unsafe fn destroy_tray(&mut self) {
        if !self.tray_data.hWnd.is_null() {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.tray_data);
            self.tray_data.hWnd = HWND::default();
        }
        if !self.tray_menu.is_null() {
            let _ = DestroyMenu(self.tray_menu);
            self.tray_menu = HMENU::default();
        }
    }

    unsafe fn show_tray_menu(&self) {
        let mut point: POINT = zeroed();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(self.hwnd);
        let _ = TrackPopupMenu(
            self.tray_menu,
            TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            self.hwnd,
            None,
        );
        let _ = PostMessageW(Some(self.hwnd), 0, WPARAM(0), LPARAM(0));
    }

    fn on_hotkey_triggered(&mut self) {
        if self.state_machine.current_state() != AppState::Idle {
            return;
        }
        self.state_machine.transition(AppEvent::HotkeyTriggered);
        if self.capture_screen() {
            self.state_machine.transition(AppEvent::FrameAcquired);
            self.show_overlay();
        } else {
            self.state_machine.transition(AppEvent::CancelRequested);
        }
    }

    fn capture_screen(&mut self) -> bool {
        self.reset_capture_session();
        match capture_region(
            self.bounds,
            Rect {
                x: 0,
                y: 0,
                w: self.screen_size.w,
                h: self.screen_size.h,
            },
        ) {
            Some(image) => {
                self.captured = Some(image);
                true
            }
            None => false,
        }
    }

    fn show_overlay(&self) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                None,
                self.bounds.x,
                self.bounds.y,
                self.bounds.w,
                self.bounds.h,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(Some(self.hwnd));
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    fn hide_overlay(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn set_passthrough_region(&mut self, region: Option<Rect>, overlay_regions: Vec<Rect>) {
        self.passthrough_region = region.filter(|rect| rect.w > 0 && rect.h > 0);
        self.overlay_regions = overlay_regions;
        self.apply_window_region();
        self.update_mouse_hook();
    }

    fn apply_window_region(&mut self) {
        unsafe {
            let Some(hole) = self.passthrough_region else {
                self.overlay_regions.clear();
                SetWindowRgn(self.hwnd, None, true);
                self.invalidate();
                return;
            };

            let region = CreateRectRgn(0, 0, 0, 0);
            if region.is_null() {
                return;
            }

            const BORDER: i32 = 2;
            add_rect_to_region(
                region,
                self.screen_size,
                hole.x - BORDER,
                hole.y - BORDER,
                hole.w + BORDER * 2,
                BORDER,
            );
            add_rect_to_region(
                region,
                self.screen_size,
                hole.x - BORDER,
                hole.y + hole.h,
                hole.w + BORDER * 2,
                BORDER,
            );
            add_rect_to_region(
                region,
                self.screen_size,
                hole.x - BORDER,
                hole.y,
                BORDER,
                hole.h,
            );
            add_rect_to_region(
                region,
                self.screen_size,
                hole.x + hole.w,
                hole.y,
                BORDER,
                hole.h,
            );

            for rect in &self.overlay_regions {
                add_rect_to_region(region, self.screen_size, rect.x, rect.y, rect.w, rect.h);
            }

            if SetWindowRgn(self.hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(HGDIOBJ::from(region));
            }
            self.invalidate();
        }
    }

    fn update_mouse_hook(&mut self) {
        unsafe {
            if self.passthrough_region.is_some() && self.mouse_hook.is_null() {
                let hinstance = GetModuleHandleW(PCWSTR::null()).ok().map(HINSTANCE::from);
                match SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), hinstance, 0) {
                    Ok(hook) => self.mouse_hook = hook,
                    Err(error) => {
                        append_log(&format!("failed to install low-level mouse hook: {error}"))
                    }
                }
            } else if self.passthrough_region.is_none() && !self.mouse_hook.is_null() {
                let _ = UnhookWindowsHookEx(self.mouse_hook);
                self.mouse_hook = HHOOK::default();
            }
        }
    }

    fn point_in_passthrough_region(&self, point: Point) -> bool {
        let Some(rect) = self.passthrough_region else {
            return false;
        };
        const BORDER: i32 = 2;
        point.x >= rect.x + BORDER
            && point.x < rect.x + rect.w - BORDER
            && point.y >= rect.y + BORDER
            && point.y < rect.y + rect.h - BORDER
    }

    fn cursor_over_toolbar_guard(&self, guard: f32) -> bool {
        if self.toolbar_w <= 0.0 || self.toolbar_h <= 0.0 {
            return false;
        }
        unsafe {
            let mut cursor: POINT = zeroed();
            if GetCursorPos(&mut cursor).is_err() {
                return false;
            }
            let cx = (cursor.x - self.bounds.x) as f32;
            let cy = (cursor.y - self.bounds.y) as f32;
            cx >= self.toolbar_x - guard
                && cx <= self.toolbar_x + self.toolbar_w + guard
                && cy >= self.toolbar_y - guard
                && cy <= self.toolbar_y + self.toolbar_h + guard
        }
    }

    fn post_scroll_at(&mut self, screen_point: Point, wheel_delta: i32) -> bool {
        unsafe {
            let point = POINT {
                x: screen_point.x,
                y: screen_point.y,
            };
            let mut target = WindowFromPoint(point);
            if !self.hwnd.is_null()
                && !target.is_null()
                && (target == self.hwnd || GetAncestor(target, GA_ROOT) == self.hwnd)
            {
                target = find_window_from_point_excluding(point, self.hwnd);
            }
            if target.is_null() {
                if !self.last_scroll_target.is_null()
                    && IsWindow(Some(self.last_scroll_target)).0 != 0
                {
                    target = self.last_scroll_target;
                } else {
                    append_long_log(&format!(
                        "scrollAt failed stage=no-target point={},{} delta={}",
                        screen_point.x, screen_point.y, wheel_delta
                    ));
                    return false;
                }
            }

            self.last_scroll_target = target;
            let root = GetAncestor(target, GA_ROOT);
            if !root.is_null() {
                let _ = SetForegroundWindow(root);
            }

            let wparam = WPARAM((wheel_delta as i16 as u16 as usize) << 16);
            let posted = PostMessageW(
                Some(target),
                WM_MOUSEWHEEL,
                wparam,
                make_lparam(screen_point.x, screen_point.y),
            )
            .is_ok();
            append_long_log(&format!(
                "scrollAt post result={} point={},{} delta={} target={:?} root={:?}",
                posted, screen_point.x, screen_point.y, wheel_delta, target, root
            ));
            posted
        }
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    fn on_mouse_down(&mut self, button_left: bool, x: i32, y: i32) {
        let p = PointF {
            x: x as f32,
            y: y as f32,
        };
        let state = self.state_machine.current_state();
        if state == AppState::Selecting {
            if button_left {
                self.is_dragging = true;
                self.drag_start = p;
                self.drag_current = p;
                unsafe {
                    SetCapture(self.hwnd);
                }
            } else {
                self.cancel_session();
            }
            self.invalidate();
            return;
        }

        if state != AppState::Annotating {
            return;
        }

        if !button_left {
            self.cancel_session();
            return;
        }

        if let Some(button_type) = self.toolbar_button_at(p.x, p.y) {
            self.pressed_toolbar_button = Some(button_type);
            self.update_toolbar_hover(p);
            self.invalidate();
            return;
        }

        if self.hit_test_style_panel(p.x, p.y) {
            self.invalidate();
            return;
        }

        if self.active_tool != AnnotationTool::None && !self.is_long_capture_active {
            self.start_annotation(p);
            self.invalidate();
        }
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        let p = PointF {
            x: x as f32,
            y: y as f32,
        };
        match self.state_machine.current_state() {
            AppState::Selecting if self.is_dragging => {
                self.drag_current = p;
                self.invalidate();
            }
            AppState::Annotating => {
                let hover_changed = self.update_toolbar_hover(p);
                if self.is_drawing_annotation {
                    self.update_annotation(p);
                    self.invalidate();
                } else if hover_changed {
                    self.invalidate();
                }
            }
            _ => {}
        }
    }

    fn on_mouse_up(&mut self, button_left: bool, x: i32, y: i32) {
        if !button_left {
            return;
        }
        let p = PointF {
            x: x as f32,
            y: y as f32,
        };
        let state = self.state_machine.current_state();
        if state == AppState::Selecting && self.is_dragging {
            unsafe {
                if GetCapture() == self.hwnd {
                    let _ = ReleaseCapture();
                }
            }
            self.is_dragging = false;
            self.drag_current = p;
            let rect = drag_rect(self.drag_start, self.drag_current);
            if rect.w > 5 && rect.h > 5 {
                self.state_machine.set_selected_region(rect);
                self.state_machine.transition(AppEvent::MouseUp);
                self.build_toolbar();
            }
            self.invalidate();
        } else if state == AppState::Annotating {
            if let Some(button_type) = self.pressed_toolbar_button.take() {
                if self.toolbar_button_at(p.x, p.y) == Some(button_type) {
                    self.on_toolbar_click(button_type);
                }
                self.invalidate();
                return;
            }
            if self.is_drawing_annotation {
                self.finish_annotation(p);
                self.invalidate();
            }
        }
    }

    fn on_mouse_wheel(&mut self, wheel_delta: i32, x: i32, y: i32) {
        if self.is_long_capture_active && self.state_machine.current_state() == AppState::Annotating
        {
            if wheel_delta < 0 {
                self.forward_long_scroll(wheel_delta, Point { x, y });
            }
        }
    }

    fn on_key_down(&mut self, key: u32) {
        if key == VK_ESCAPE.0 as u32 {
            self.cancel_session();
            return;
        }
        let ctrl = unsafe { GetKeyState(0x11) < 0 };
        let shift = unsafe { GetKeyState(0x10) < 0 };
        if ctrl && key == 'Z' as u32 {
            if shift {
                self.command_history.redo(&mut self.annotations);
            } else {
                self.command_history.undo(&mut self.annotations);
            }
            self.invalidate();
        } else if ctrl && key == 'S' as u32 {
            if self.state_machine.current_state() == AppState::Annotating {
                self.save_to_file();
            }
        } else if ctrl && key == 'C' as u32 {
            if self.state_machine.current_state() == AppState::Annotating {
                self.save_to_clipboard();
            }
        }
    }

    fn on_timer(&mut self) {
        self.poll_long_capture_worker();
        if self.long_auto_scroll_active && Instant::now() >= self.next_long_auto_scroll {
            let now = Instant::now();
            self.next_long_auto_scroll = now + LONG_AUTO_SCROLL_INTERVAL;
            let local_point = self.long_scroll_point();
            let screen_point = Point {
                x: self.bounds.x + local_point.x,
                y: self.bounds.y + local_point.y,
            };
            let wheel_delta = (LONG_AUTO_SCROLL_DELTA * 120.0) as i32;
            if wheel_delta < 0 && self.post_scroll_at(screen_point, wheel_delta) {
                let (seq, due) = self.schedule_long_frame_capture(
                    now,
                    LONG_AUTO_CAPTURE_DELAY,
                    LONG_AUTO_CAPTURE_INTERVAL,
                );
                append_long_log(&format!(
                    "auto-scroll step seq={} delta={} capture_due_ms={} next_scroll_ms={}",
                    seq,
                    wheel_delta,
                    due.saturating_duration_since(now).as_millis(),
                    LONG_AUTO_SCROLL_INTERVAL.as_millis()
                ));
            } else {
                self.long_auto_stalls += 1;
                append_long_log(&format!(
                    "auto-scroll stall count={} reason=scroll-forward-failed",
                    self.long_auto_stalls
                ));
                if self.long_auto_stalls >= 5 {
                    self.long_auto_scroll_active = false;
                    self.refresh_long_preview();
                    self.invalidate();
                }
            }
        }
        self.run_pending_long_capture();
        self.poll_long_capture_worker();
    }

    fn cancel_session(&mut self) {
        self.state_machine.transition(AppEvent::Escape);
        self.hide_overlay();
        self.reset_capture_session();
    }

    fn reset_capture_session(&mut self) {
        self.annotations.clear();
        self.command_history.clear();
        self.is_dragging = false;
        self.is_drawing_annotation = false;
        self.active_freehand_points.clear();
        self.tool_buttons.clear();
        self.pressed_toolbar_button = None;
        self.toolbar_x = 0.0;
        self.toolbar_y = 0.0;
        self.toolbar_w = 0.0;
        self.toolbar_h = 0.0;
        self.style_panel_x = 0.0;
        self.style_panel_y = 0.0;
        self.style_panel_w = 0.0;
        self.style_panel_h = 0.0;
        self.is_long_result = false;
        self.is_long_capture_active = false;
        self.long_auto_scroll_active = false;
        self.long_auto_stalls = 0;
        self.next_long_auto_scroll = Instant::now();
        self.next_long_auto_preview_render = self.next_long_auto_scroll;
        self.pending_long_frame_capture = false;
        self.long_needs_trailing_capture = false;
        self.long_scroll_seq = 0;
        self.long_pending_scroll_seq = 0;
        self.long_pending_scroll_at = self.next_long_auto_scroll;
        self.long_current_scroll_seq = 0;
        self.long_current_scroll_at = self.next_long_auto_scroll;
        self.long_last_appended_scroll_seq = 0;
        self.long_last_frame_capture = self.next_long_auto_scroll;
        self.long_source_region = Rect::default();
        self.long_stitcher = LongScreenshotStitcher::default();
        self.long_capture_generation = self.long_capture_generation.saturating_add(1);
        self.long_capture_in_flight = false;
        self.long_capture_in_flight_seq = 0;
        self.long_capture_in_flight_needs_trailing = false;
        self.long_capture_in_flight_trailing_due = self.next_long_auto_scroll;
        self.long_preview_thumbnail = None;
        self.long_preview_thumbnail_rect = Rect::default();
        self.last_scroll_target = HWND::default();
        self.long_background = None;
        self.set_passthrough_region(None, Vec::new());
    }

    fn build_toolbar(&mut self) {
        self.tool_buttons.clear();
        self.style_panel_x = 0.0;
        self.style_panel_y = 0.0;
        self.style_panel_w = 0.0;
        self.style_panel_h = 0.0;
        let sel = self.state_machine.selected_region();

        if self.is_long_result {
            let total_w = 5.0 * LONG_TOOLBAR_BTN_SIZE;
            let total_h = LONG_TOOLBAR_BTN_SIZE;
            let anchor = if self.long_source_region.w > 0 {
                self.long_source_region
            } else {
                sel
            };
            let sw = self.screen_size.w as f32;
            let sh = self.screen_size.h as f32;
            let tx = (anchor.x + anchor.w) as f32 - total_w;
            let ty = (anchor.y + anchor.h) as f32 + 10.0;
            self.toolbar_x = tx.clamp(8.0, (sw - total_w - 8.0).max(8.0));
            self.toolbar_y = ty.clamp(8.0, (sh - total_h - 8.0).max(8.0));
            self.toolbar_w = total_w;
            self.toolbar_h = total_h;
            let mut bx = self.toolbar_x;
            for button_type in [
                ToolButtonType::Edit,
                ToolButtonType::AutoScroll,
                ToolButtonType::Save,
                ToolButtonType::Cancel,
                ToolButtonType::Confirm,
            ] {
                self.tool_buttons.push(ToolButton {
                    x: bx,
                    y: self.toolbar_y,
                    w: LONG_TOOLBAR_BTN_SIZE,
                    h: LONG_TOOLBAR_BTN_SIZE,
                    button_type,
                    hovered: false,
                });
                bx += LONG_TOOLBAR_BTN_SIZE;
            }
            return;
        }

        let tool_buttons = [
            ToolButtonType::Rectangle,
            ToolButtonType::Circle,
            ToolButtonType::AddText,
            ToolButtonType::NumberMarker,
            ToolButtonType::Pen,
            ToolButtonType::Arrow,
            ToolButtonType::Line,
            ToolButtonType::DashedLine,
            ToolButtonType::Mosaic,
            ToolButtonType::Gif,
            ToolButtonType::Pin,
            ToolButtonType::Ocr,
            ToolButtonType::LongScreenshot,
        ];
        let action_buttons = [
            ToolButtonType::Undo,
            ToolButtonType::Save,
            ToolButtonType::Cancel,
            ToolButtonType::Confirm,
        ];
        let num_buttons = (tool_buttons.len() + action_buttons.len()) as f32;
        let normal_gaps =
            (tool_buttons.len().saturating_sub(1) + action_buttons.len().saturating_sub(1)) as f32;
        let group_gaps = 1.0;
        let total_w = TOOLBAR_PAD * 2.0
            + num_buttons * BTN_SIZE
            + normal_gaps * BTN_GAP
            + group_gaps * TOOLBAR_GROUP_GAP;
        let total_h = TOOLBAR_PAD * 2.0 + BTN_SIZE;
        let mut tx = (sel.x + sel.w) as f32 - total_w;
        let mut ty = (sel.y + sel.h) as f32 + TOOLBAR_GAP;
        let sw = self.screen_size.w as f32;
        let sh = self.screen_size.h as f32;
        tx = tx.clamp(0.0, (sw - total_w).max(0.0));
        if ty + total_h > sh {
            ty = (sel.y as f32 - TOOLBAR_GAP - total_h).max(0.0);
        }
        self.toolbar_x = tx;
        self.toolbar_y = ty;
        self.toolbar_w = total_w;
        self.toolbar_h = total_h;

        let mut bx = tx + TOOLBAR_PAD;
        let by = ty + TOOLBAR_PAD;
        for button_type in tool_buttons {
            push_toolbar_button(&mut self.tool_buttons, &mut bx, by, button_type);
        }
        bx += TOOLBAR_GROUP_GAP - BTN_GAP;
        for button_type in action_buttons {
            push_toolbar_button(&mut self.tool_buttons, &mut bx, by, button_type);
        }

        let style_w = STYLE_PANEL_LABEL_W + STYLE_PANEL_GAP + STYLE_PANEL_CONTROLS_W;
        let style_x = (tx + (total_w - style_w) * 0.5).clamp(0.0, (sw - style_w).max(0.0));
        let below_y = ty + total_h + STYLE_PANEL_GAP;
        let style_y = if below_y + STYLE_PANEL_H <= sh {
            below_y
        } else {
            (ty - STYLE_PANEL_GAP - STYLE_PANEL_H).clamp(0.0, (sh - STYLE_PANEL_H).max(0.0))
        };
        self.style_panel_x = style_x;
        self.style_panel_y = style_y;
        self.style_panel_w = style_w.min(sw);
        self.style_panel_h = STYLE_PANEL_H;
    }

    fn toolbar_button_at(&self, x: f32, y: f32) -> Option<ToolButtonType> {
        self.tool_buttons
            .iter()
            .find(|button| point_in_button(x, y, button))
            .copied()
            .map(|button| button.button_type)
    }

    fn hit_test_style_panel(&self, x: f32, y: f32) -> bool {
        self.style_panel_w > 0.0
            && self.style_panel_h > 0.0
            && x >= self.style_panel_x
            && x <= self.style_panel_x + self.style_panel_w
            && y >= self.style_panel_y
            && y <= self.style_panel_y + self.style_panel_h
    }

    fn update_toolbar_hover(&mut self, point: PointF) -> bool {
        let mut changed = false;
        for button in &mut self.tool_buttons {
            let hovered = point_in_button(point.x, point.y, button);
            if button.hovered != hovered {
                button.hovered = hovered;
                changed = true;
            }
        }
        changed
    }

    fn on_toolbar_click(&mut self, button_type: ToolButtonType) {
        match button_type {
            ToolButtonType::Edit | ToolButtonType::Rectangle => {
                self.active_tool = AnnotationTool::Rectangle
            }
            ToolButtonType::Pen => self.active_tool = AnnotationTool::Freehand,
            ToolButtonType::Arrow => self.active_tool = AnnotationTool::Arrow,
            ToolButtonType::Line => self.active_tool = AnnotationTool::Line,
            ToolButtonType::LongScreenshot => {
                self.start_long_screenshot();
            }
            ToolButtonType::AutoScroll => {
                let was_active = self.long_auto_scroll_active;
                self.long_auto_scroll_active = !self.long_auto_scroll_active;
                self.long_auto_stalls = 0;
                let now = Instant::now();
                self.next_long_auto_scroll = now;
                self.next_long_auto_preview_render = now;
                if was_active {
                    self.refresh_long_preview();
                } else {
                    self.build_toolbar();
                    self.set_passthrough_region(
                        Some(self.long_source_region),
                        self.long_screenshot_overlay_regions(),
                    );
                }
            }
            ToolButtonType::Undo => self.command_history.undo(&mut self.annotations),
            ToolButtonType::Save => {
                self.save_to_file();
            }
            ToolButtonType::Confirm => {
                self.save_to_clipboard();
            }
            ToolButtonType::Cancel => self.cancel_session(),
            ToolButtonType::Pin => {
                self.pin_selection();
            }
            ToolButtonType::Gif => {
                self.record_gif_selection();
            }
            ToolButtonType::Circle
            | ToolButtonType::AddText
            | ToolButtonType::NumberMarker
            | ToolButtonType::DashedLine
            | ToolButtonType::Mosaic
            | ToolButtonType::Ocr => {}
        }
    }

    fn start_annotation(&mut self, point: PointF) {
        self.is_drawing_annotation = true;
        self.annotation_start = point;
        self.annotation_current = point;
        self.active_freehand_points.clear();
        if self.active_tool == AnnotationTool::Freehand {
            append_freehand_point(&mut self.active_freehand_points, point.x, point.y);
        }
    }

    fn update_annotation(&mut self, point: PointF) {
        self.annotation_current = point;
        if self.active_tool == AnnotationTool::Freehand {
            append_freehand_point(&mut self.active_freehand_points, point.x, point.y);
        }
    }

    fn finish_annotation(&mut self, point: PointF) {
        self.is_drawing_annotation = false;
        self.annotation_current = point;
        let dx = self.annotation_current.x - self.annotation_start.x;
        let dy = self.annotation_current.y - self.annotation_start.y;
        if self.active_tool != AnnotationTool::Freehand && dx.abs() < 3.0 && dy.abs() < 3.0 {
            return;
        }

        let annotation = match self.active_tool {
            AnnotationTool::Rectangle => {
                let x = self.annotation_start.x.min(self.annotation_current.x);
                let y = self.annotation_start.y.min(self.annotation_current.y);
                Annotation::Rect(RectAnnotation {
                    bounds: RectF {
                        x,
                        y,
                        w: dx.abs(),
                        h: dy.abs(),
                    },
                    color: self.annotation_color,
                    thickness: self.annotation_thickness,
                    filled: false,
                })
            }
            AnnotationTool::Arrow => Annotation::Arrow(ArrowAnnotation {
                start: self.annotation_start,
                end: self.annotation_current,
                color: self.annotation_color,
                thickness: self.annotation_thickness,
                head_size: 12.0,
            }),
            AnnotationTool::Line => Annotation::Line(LineAnnotation {
                start: self.annotation_start,
                end: self.annotation_current,
                color: self.annotation_color,
                thickness: self.annotation_thickness,
            }),
            AnnotationTool::Freehand => {
                append_freehand_point(
                    &mut self.active_freehand_points,
                    self.annotation_current.x,
                    self.annotation_current.y,
                );
                if self.active_freehand_points.len() < 2 {
                    return;
                }
                Annotation::Freehand(FreehandAnnotation {
                    points: smooth_freehand_points(&self.active_freehand_points),
                    color: self.annotation_color,
                    thickness: self.annotation_thickness,
                })
            }
            _ => return,
        };
        self.command_history
            .execute(annotation, &mut self.annotations);
        self.active_freehand_points.clear();
    }

    fn start_long_screenshot(&mut self) -> bool {
        if self.is_long_capture_active || self.is_long_result {
            return false;
        }
        let selected = self.state_machine.selected_region();
        let Some(captured) = self.captured.clone() else {
            return false;
        };
        let Some(first_frame) = captured.crop(selected) else {
            return false;
        };

        let options = LongShotSessionOptions::tuned_for_region_height(selected.h).stitch;

        self.long_background = Some(captured);
        self.long_source_region = selected;
        self.long_stitcher = LongScreenshotStitcher::new(selected.w, options);
        self.long_stitcher
            .start(&first_frame.pixels, first_frame.height);
        self.captured = Image::from_rgba(
            self.long_stitcher.width(),
            self.long_stitcher.height(),
            self.long_stitcher.pixels().to_vec(),
        );
        self.update_long_preview_thumbnail();
        self.is_long_capture_active = true;
        self.is_long_result = true;
        self.long_auto_scroll_active = false;
        self.long_auto_stalls = 0;
        self.pending_long_frame_capture = false;
        self.long_needs_trailing_capture = false;
        self.long_scroll_seq = 0;
        self.long_pending_scroll_seq = 0;
        let now = Instant::now();
        self.long_pending_scroll_at = now;
        self.long_current_scroll_seq = 0;
        self.long_current_scroll_at = now;
        self.long_last_appended_scroll_seq = 0;
        self.long_last_frame_capture = now;
        self.next_long_auto_scroll = now;
        self.next_long_auto_preview_render = now;
        self.long_capture_generation = self.long_capture_generation.saturating_add(1);
        self.long_capture_in_flight = false;
        self.long_capture_in_flight_seq = 0;
        self.long_capture_in_flight_needs_trailing = false;
        self.long_capture_in_flight_trailing_due = now;
        self.long_preview_thumbnail = None;
        self.long_preview_thumbnail_rect = Rect::default();
        self.last_scroll_target = HWND::default();
        self.annotations.clear();
        self.command_history.clear();
        self.state_machine.set_selected_region(
            self.fit_long_preview_rect(self.long_stitcher.width(), self.long_stitcher.height()),
        );
        self.build_toolbar();
        self.set_passthrough_region(
            Some(self.long_source_region),
            self.long_screenshot_overlay_regions(),
        );
        append_long_log(&format!(
            "long-session start source={},{} {}x{}",
            self.long_source_region.x,
            self.long_source_region.y,
            self.long_source_region.w,
            self.long_source_region.h
        ));
        true
    }

    fn forward_long_scroll(&mut self, wheel_delta: i32, local_point: Point) -> bool {
        if !self.is_long_capture_active || self.long_source_region.w <= 0 || wheel_delta >= 0 {
            return false;
        }
        let local_point = if self.long_source_region.contains(local_point) {
            local_point
        } else {
            self.long_scroll_point()
        };
        let screen_point = Point {
            x: self.bounds.x + local_point.x,
            y: self.bounds.y + local_point.y,
        };
        let started = Instant::now();
        if !self.post_scroll_at(screen_point, wheel_delta) {
            append_long_log(&format!(
                "scroll-forward failed delta={} point={},{}",
                wheel_delta, screen_point.x, screen_point.y
            ));
            return false;
        }
        let (seq, due) = self.schedule_long_frame_capture(
            started,
            LONG_CAPTURE_DELAY,
            LONG_MIN_CAPTURE_INTERVAL,
        );
        append_long_log(&format!(
            "scroll-forward ok seq={} delta={} point={},{} capture_due_ms={} trailing_due_ms={}",
            seq,
            wheel_delta,
            screen_point.x,
            screen_point.y,
            due.saturating_duration_since(Instant::now()).as_millis(),
            LONG_TRAILING_CAPTURE_DELAY.as_millis()
        ));
        true
    }

    fn handle_native_long_scroll(&mut self, wheel_delta: i32, screen_point: Point) {
        if !self.is_long_capture_active || wheel_delta >= 0 {
            return;
        }
        let client_point = Point {
            x: screen_point.x - self.bounds.x,
            y: screen_point.y - self.bounds.y,
        };
        if self.point_in_passthrough_region(client_point) {
            let started = Instant::now();
            let (seq, due) = self.schedule_long_frame_capture(
                started,
                LONG_NATIVE_CAPTURE_DELAY,
                LONG_MIN_CAPTURE_INTERVAL,
            );
            append_long_log(&format!(
                "scroll-observed ok seq={} delta={} point={},{} capture_due_ms={} trailing_due_ms={}",
                seq,
                wheel_delta,
                screen_point.x,
                screen_point.y,
                due.saturating_duration_since(started).as_millis(),
                LONG_TRAILING_CAPTURE_DELAY.as_millis()
            ));
        }
    }

    fn schedule_long_frame_capture(
        &mut self,
        scroll_at: Instant,
        capture_delay: Duration,
        min_capture_interval: Duration,
    ) -> (u64, Instant) {
        let now = Instant::now();
        self.long_scroll_seq = self.long_scroll_seq.saturating_add(1);
        self.long_pending_scroll_seq = self.long_scroll_seq;
        self.long_pending_scroll_at = scroll_at;
        let requested_due = now + capture_delay;
        let interval_due = self.long_last_frame_capture + min_capture_interval;
        let next_due = if requested_due > interval_due {
            requested_due
        } else {
            interval_due
        };
        self.long_frame_capture_due = if self.pending_long_frame_capture {
            self.long_frame_capture_due.min(next_due)
        } else {
            next_due
        };
        self.long_trailing_capture_due = now + LONG_TRAILING_CAPTURE_DELAY;
        self.long_needs_trailing_capture = true;
        self.pending_long_frame_capture = true;
        append_long_log(&format!(
            "long-capture scheduled seq={} due_ms={} min_interval_ms={}",
            self.long_pending_scroll_seq,
            self.long_frame_capture_due
                .saturating_duration_since(now)
                .as_millis(),
            min_capture_interval.as_millis()
        ));
        (self.long_pending_scroll_seq, self.long_frame_capture_due)
    }

    fn run_pending_long_capture(&mut self) {
        if !self.pending_long_frame_capture
            || !self.is_long_capture_active
            || self.long_capture_in_flight
        {
            return;
        }
        let now = Instant::now();
        if now < self.long_frame_capture_due {
            return;
        }

        if !self.long_auto_scroll_active && self.cursor_over_toolbar_guard(8.0) {
            self.long_frame_capture_due = now + LONG_CAPTURE_DELAY;
            return;
        }

        self.pending_long_frame_capture = false;
        self.long_current_scroll_seq = self.long_pending_scroll_seq;
        self.long_current_scroll_at = self.long_pending_scroll_at;
        let needs_trailing = self.long_needs_trailing_capture;
        let trailing_due = self.long_trailing_capture_due;
        self.long_needs_trailing_capture = false;
        self.long_last_frame_capture = now;

        append_long_log(&format!(
            "capture-due seq={} event_to_due_ms={} schedule_trailing={}",
            self.long_current_scroll_seq,
            now.saturating_duration_since(self.long_current_scroll_at)
                .as_millis(),
            needs_trailing
        ));
        self.dispatch_long_capture_request(needs_trailing, trailing_due);
    }

    fn dispatch_long_capture_request(&mut self, needs_trailing: bool, trailing_due: Instant) {
        let Some(source_region) = self.long_source_region.intersect(Rect {
            x: 0,
            y: 0,
            w: self.screen_size.w,
            h: self.screen_size.h,
        }) else {
            append_long_log("long-capture skipped: source region outside desktop");
            self.finish_long_capture(false, needs_trailing, trailing_due);
            return;
        };
        if self.long_stitcher.height() + source_region.h >= LONG_MAX_OUTPUT_HEIGHT {
            self.long_auto_scroll_active = false;
            self.finish_long_capture(false, needs_trailing, trailing_due);
            return;
        }

        let request = LongCaptureRequest {
            generation: self.long_capture_generation,
            seq: self.long_current_scroll_seq,
            desktop_bounds: self.bounds,
            overlay_hwnd: self.hwnd.0 as isize,
            region: source_region,
        };

        if let Some(worker) = &self.long_capture_worker {
            if worker.request(request) {
                self.long_capture_in_flight = true;
                self.long_capture_in_flight_seq = request.seq;
                self.long_capture_in_flight_needs_trailing = needs_trailing;
                self.long_capture_in_flight_trailing_due = trailing_due;
                return;
            }
        }

        let frame = capture_region(request.desktop_bounds, request.region)
            .or_else(|| capture_covered_window(request.desktop_bounds, self.hwnd, request.region));
        let appended = frame
            .map(|frame| self.append_long_screenshot_frame(frame))
            .unwrap_or_else(|| {
                append_long_log("long-capture failed");
                false
            });
        self.finish_long_capture(appended, needs_trailing, trailing_due);
    }

    fn poll_long_capture_worker(&mut self) {
        let responses = self
            .long_capture_worker
            .as_ref()
            .map(LongCaptureWorker::drain_responses)
            .unwrap_or_default();

        for response in responses {
            self.handle_long_capture_response(response);
        }
    }

    fn wait_for_long_capture_response(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while self.long_capture_in_flight {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            if remaining.is_zero() {
                break;
            }

            let response = self
                .long_capture_worker
                .as_ref()
                .and_then(|worker| worker.wait_for_response(remaining));
            let Some(response) = response else {
                break;
            };
            self.handle_long_capture_response(response);
        }
    }

    fn handle_long_capture_response(&mut self, response: LongCaptureResponse) {
        if !self.long_capture_in_flight
            || response.generation != self.long_capture_generation
            || response.seq != self.long_capture_in_flight_seq
        {
            return;
        }

        self.long_capture_in_flight = false;
        let needs_trailing = self.long_capture_in_flight_needs_trailing;
        let trailing_due = self.long_capture_in_flight_trailing_due;
        let appended = response
            .frame
            .map(|frame| self.append_long_screenshot_frame(frame))
            .unwrap_or_else(|| {
                append_long_log("long-capture failed");
                false
            });
        self.finish_long_capture(appended, needs_trailing, trailing_due);
    }

    fn flush_long_capture_for_export(&mut self) {
        if !self.is_long_capture_active {
            return;
        }

        self.long_auto_scroll_active = false;
        self.poll_long_capture_worker();
        self.wait_for_long_capture_response(Duration::from_millis(500));

        self.pending_long_frame_capture = false;
        self.long_needs_trailing_capture = false;
        self.long_capture_in_flight = false;
        self.long_capture_generation = self.long_capture_generation.saturating_add(1);

        let now = Instant::now();
        self.long_scroll_seq = self.long_scroll_seq.saturating_add(1);
        self.long_current_scroll_seq = self.long_scroll_seq;
        self.long_pending_scroll_seq = self.long_scroll_seq;
        self.long_current_scroll_at = now;
        self.long_pending_scroll_at = now;
        self.long_last_frame_capture = now;

        let Some(source_region) = self.long_source_region.intersect(Rect {
            x: 0,
            y: 0,
            w: self.screen_size.w,
            h: self.screen_size.h,
        }) else {
            return;
        };
        if self.long_stitcher.height() + source_region.h >= LONG_MAX_OUTPUT_HEIGHT {
            return;
        }

        let appended = capture_region(self.bounds, source_region)
            .or_else(|| capture_covered_window(self.bounds, self.hwnd, source_region))
            .map(|frame| self.append_long_screenshot_frame(frame))
            .unwrap_or(false);
        if appended {
            self.refresh_long_preview();
        }
    }

    fn finish_long_capture(&mut self, appended: bool, needs_trailing: bool, trailing_due: Instant) {
        if appended {
            self.long_auto_stalls = 0;
            let render_check = Instant::now();
            let should_render_preview =
                !self.long_auto_scroll_active || render_check >= self.next_long_auto_preview_render;
            if should_render_preview {
                if self.long_auto_scroll_active {
                    self.next_long_auto_preview_render =
                        render_check + LONG_AUTO_PREVIEW_RENDER_INTERVAL;
                }
                self.refresh_long_preview();
                self.invalidate();
            } else {
                append_long_log(&format!(
                    "render-skip seq={} reason=auto-preview-throttle next_render_ms={}",
                    self.long_current_scroll_seq,
                    self.next_long_auto_preview_render
                        .saturating_duration_since(render_check)
                        .as_millis()
                ));
            }
            return;
        }

        if needs_trailing {
            if self.long_auto_scroll_active {
                self.long_auto_stalls += 1;
                append_long_log(&format!(
                    "auto-scroll stall seq={} count={} reason=no-append-trailing-suppressed",
                    self.long_current_scroll_seq, self.long_auto_stalls
                ));
                if self.long_auto_stalls >= 5 {
                    self.long_auto_scroll_active = false;
                    self.refresh_long_preview();
                    self.invalidate();
                }
            } else {
                let after_capture = Instant::now();
                let requested_due = after_capture + LONG_CAPTURE_DELAY;
                let interval_due = self.long_last_frame_capture + LONG_MIN_CAPTURE_INTERVAL;
                self.long_frame_capture_due =
                    max_instant(max_instant(requested_due, interval_due), trailing_due);
                self.pending_long_frame_capture = true;
            }
        } else if self.long_auto_scroll_active {
            self.long_auto_stalls += 1;
            if self.long_auto_stalls >= 5 {
                self.long_auto_scroll_active = false;
                self.refresh_long_preview();
                self.invalidate();
            } else {
                self.next_long_auto_scroll = Instant::now() + LONG_AUTO_SCROLL_INTERVAL;
            }
        }
    }

    fn append_long_screenshot_frame(&mut self, mut frame: Image) -> bool {
        if !self.is_long_capture_active || self.long_source_region.w <= 0 {
            return false;
        }
        if frame.width != self.long_source_region.w {
            append_long_log(&format!(
                "long-capture wrong width: got={} expected={}",
                frame.width, self.long_source_region.w
            ));
            return false;
        }
        if self.long_current_scroll_seq != 0
            && self.long_last_appended_scroll_seq == self.long_current_scroll_seq
        {
            append_long_log(&format!(
                "append skipped seq={} reason=same-scroll-already-appended",
                self.long_current_scroll_seq
            ));
            return false;
        }
        frame.height = trim_trailing_capture_dropout(&mut frame.pixels, frame.width, frame.height);
        let allow_acceptable_match = self.long_current_scroll_seq == 0
            || self.long_last_appended_scroll_seq != self.long_current_scroll_seq;
        let result = self
            .long_stitcher
            .append(&frame.pixels, frame.height, allow_acceptable_match);
        if !result.appended {
            append_long_log(&format!(
                "long-scroll ignored: duplicate={} reliable={} allow_acceptable={} overlap={} score={:.2} second={:.2}",
                result.duplicate,
                result.reliable,
                allow_acceptable_match,
                result.overlap_rows,
                result.score,
                result.second_best_score
            ));
            return false;
        }
        self.long_last_appended_scroll_seq = self.long_current_scroll_seq;
        append_long_log(&format!(
            "long-append ok seq={} output={}x{} appended_rows={} overlap={} score={:.2}",
            self.long_current_scroll_seq,
            self.long_stitcher.width(),
            self.long_stitcher.height(),
            result.appended_rows,
            result.overlap_rows,
            result.score
        ));
        true
    }

    fn refresh_long_preview(&mut self) {
        if self.long_stitcher.empty() {
            return;
        }
        self.captured = Image::from_rgba(
            self.long_stitcher.width(),
            self.long_stitcher.height(),
            self.long_stitcher.pixels().to_vec(),
        );
        self.update_long_preview_thumbnail();
        self.state_machine.set_selected_region(
            self.fit_long_preview_rect(self.long_stitcher.width(), self.long_stitcher.height()),
        );
        self.build_toolbar();
        self.set_passthrough_region(
            Some(self.long_source_region),
            self.long_screenshot_overlay_regions(),
        );
    }

    fn long_scroll_point(&self) -> Point {
        let region = if self.long_source_region.w > 0 {
            self.long_source_region
        } else {
            self.state_machine.selected_region()
        };
        Point {
            x: region.x + region.w / 2,
            y: region.y + region.h / 2,
        }
    }

    fn fit_long_preview_rect(&self, image_w: i32, image_h: i32) -> Rect {
        if self.long_source_region.w > 0 && self.long_source_region.h > 0 {
            return self.long_source_region;
        }
        let viewport_h = image_h.min(self.screen_size.h - 148);
        Rect {
            x: (self.screen_size.w - image_w) / 2,
            y: 56.max((self.screen_size.h - viewport_h) / 2 - 10),
            w: image_w,
            h: viewport_h,
        }
    }

    fn long_thumbnail_rect_for(&self, image: &Image) -> Option<Rect> {
        if self.long_source_region.w <= 0 || self.long_source_region.h <= 0 {
            return None;
        }
        let source = self.long_source_region;
        let available_h = (self.screen_size.h - 32).max(1);
        let thumb_h = (source.h + 96).max(160).min(available_h);
        let thumb_w = ((thumb_h as f32 * image.width as f32 / image.height.max(1) as f32).round()
            as i32)
            .clamp(72, 132);
        let x = if source.x - thumb_w - 18 >= 8 {
            source.x - thumb_w - 18
        } else {
            (source.x + source.w + 18).min(self.screen_size.w - thumb_w - 8)
        };
        let max_y = (self.screen_size.h - thumb_h - 8).max(8);
        let y = (source.y + (source.h - thumb_h) / 2).clamp(8, max_y);
        Some(Rect {
            x,
            y,
            w: thumb_w,
            h: thumb_h,
        })
    }

    fn update_long_preview_thumbnail(&mut self) {
        let Some(captured) = &self.captured else {
            self.long_preview_thumbnail = None;
            self.long_preview_thumbnail_rect = Rect::default();
            return;
        };
        let Some(rect) = self.long_thumbnail_rect_for(captured) else {
            self.long_preview_thumbnail = None;
            self.long_preview_thumbnail_rect = Rect::default();
            return;
        };

        let mut thumbnail = Image::new(rect.w, rect.h);
        thumbnail.blit_scaled(
            captured,
            Rect {
                x: 0,
                y: 0,
                w: rect.w,
                h: rect.h,
            },
        );
        self.long_preview_thumbnail = Some(thumbnail);
        self.long_preview_thumbnail_rect = rect;
    }

    fn long_screenshot_overlay_regions(&self) -> Vec<Rect> {
        let mut regions = Vec::new();
        if self.toolbar_w > 0.0 && self.toolbar_h > 0.0 {
            regions.push(padded_rect(
                rect_from_f32(
                    self.toolbar_x,
                    self.toolbar_y,
                    self.toolbar_w,
                    self.toolbar_h,
                ),
                4,
            ));
        }

        if self.long_source_region.w <= 0 || self.long_source_region.h <= 0 {
            return regions;
        }

        if self.long_preview_thumbnail.is_some() {
            regions.push(padded_rect(self.long_preview_thumbnail_rect, 6));
        }

        regions
    }

    fn render_output_pixels(&self) -> Option<(Vec<u8>, i32, i32)> {
        if self.is_long_result {
            let mut image = if !self.long_stitcher.empty() {
                Image::from_rgba(
                    self.long_stitcher.width(),
                    self.long_stitcher.height(),
                    self.long_stitcher.pixels().to_vec(),
                )?
            } else {
                self.captured.clone()?
            };
            for annotation in self.annotations.annotations() {
                image.draw_annotation(annotation, 0.0, 0.0);
            }
            return Some((image.pixels, image.width, image.height));
        }

        let captured = self.captured.as_ref()?;
        let selected = self.state_machine.selected_region();
        let mut image = captured.crop(selected)?;
        for annotation in self.annotations.annotations() {
            image.draw_annotation(annotation, selected.x as f32, selected.y as f32);
        }
        Some((image.pixels, image.width, image.height))
    }

    fn save_to_clipboard(&mut self) -> bool {
        self.flush_long_capture_for_export();
        let Some((pixels, width, height)) = self.render_output_pixels() else {
            return false;
        };
        self.state_machine.transition(AppEvent::CopyRequested);
        self.hide_overlay();
        let ok = unsafe { write_clipboard_image(self.hwnd, &pixels, width, height) };
        self.state_machine.transition(AppEvent::SaveComplete);
        self.reset_capture_session();
        ok
    }

    fn save_to_file(&mut self) -> bool {
        self.flush_long_capture_for_export();
        let Some((pixels, width, height)) = self.render_output_pixels() else {
            return false;
        };
        self.hide_overlay();
        let path = unsafe { show_save_dialog(self.hwnd) };
        let Some(path) = path else {
            self.show_overlay();
            return false;
        };
        self.state_machine.transition(AppEvent::SaveRequested);
        let ok = encode_rgba_png(width, height, &pixels)
            .and_then(|png| std::fs::write(path, png).ok())
            .is_some();
        self.state_machine.transition(AppEvent::SaveComplete);
        self.reset_capture_session();
        ok
    }

    fn pin_selection(&mut self) -> bool {
        self.flush_long_capture_for_export();
        let Some((pixels, width, height)) = self.render_output_pixels() else {
            return false;
        };
        let Some(image) = Image::from_rgba(width, height, pixels) else {
            return false;
        };
        let ok = crate::windows::pindlg::spawn_pinned_image(image);
        if ok {
            self.hide_overlay();
            self.reset_capture_session();
        }
        ok
    }

    fn record_gif_selection(&mut self) -> bool {
        let selected = self.state_machine.selected_region();
        if selected.w <= 0 || selected.h <= 0 {
            return false;
        }

        self.hide_overlay();
        let path = unsafe { show_gif_save_dialog(self.hwnd) };
        let Some(path) = path else {
            self.show_overlay();
            return false;
        };

        let options = crate::windows::gifdlg::GifRecordOptions {
            fps: 10,
            max_duration: Duration::from_secs(5),
            max_cache_bytes: 150 * 1024 * 1024,
        };
        let ok =
            crate::windows::gifdlg::record_region_to_file(self.bounds, selected, path, options);
        self.reset_capture_session();
        ok
    }

    fn paint(&mut self) {
        let mut buffer = self.compose_frame();
        unsafe {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(self.hwnd, &mut ps);
            if !hdc.is_null() {
                present_image(hdc, &mut buffer);
            }
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    fn compose_frame(&self) -> Image {
        let mut frame = if self.is_long_result {
            self.long_background
                .clone()
                .unwrap_or_else(|| Image::new(self.screen_size.w, self.screen_size.h))
        } else {
            self.captured
                .clone()
                .unwrap_or_else(|| Image::new(self.screen_size.w, self.screen_size.h))
        };

        match self.state_machine.current_state() {
            AppState::Selecting => {
                if self.is_dragging {
                    let rect = drag_rect(self.drag_start, self.drag_current);
                    render_dim_mask(&mut frame, rect, DIM_COLOR);
                    draw_selection_frame(&mut frame, rect, SEL_BORDER, SEL_ACCENT);
                    render_dimension_badge(&mut frame, rect);
                } else {
                    frame.blend_rect(frame.bounds(), DIM_COLOR);
                }
            }
            AppState::Annotating => {
                if self.is_long_result {
                    self.render_long_ui(&mut frame);
                } else {
                    let selected = self.state_machine.selected_region();
                    render_dim_mask(&mut frame, selected, DIM_COLOR);
                    draw_selection_frame(&mut frame, selected, SEL_BORDER, SEL_ACCENT);
                    render_dimension_badge(&mut frame, selected);
                    for annotation in self.annotations.annotations() {
                        frame.draw_annotation(annotation, 0.0, 0.0);
                    }
                    self.draw_active_annotation(&mut frame);
                    self.render_toolbar(&mut frame);
                }
            }
            _ => {}
        }
        frame
    }

    fn render_long_ui(&self, frame: &mut Image) {
        frame.blend_rect(frame.bounds(), LONG_DIM_COLOR);
        let source = self.long_source_region;
        draw_selection_frame(frame, source, LONG_BORDER, LONG_BORDER);
        if let Some(thumbnail) = &self.long_preview_thumbnail {
            let rect = self.long_preview_thumbnail_rect;
            frame.blend_rect(
                Rect {
                    x: rect.x + 3,
                    y: rect.y + 3,
                    w: rect.w,
                    h: rect.h,
                },
                Color::rgba(0, 0, 0, 52),
            );
            frame.blend_rect(rect, LONG_PANEL);
            frame.blit_scaled(thumbnail, rect);
            frame.draw_rect_outline(
                Rect {
                    x: rect.x - 1,
                    y: rect.y - 1,
                    w: rect.w + 2,
                    h: rect.h + 2,
                },
                Color::rgba(255, 255, 255, 160),
                1.0,
            );
        }
        self.render_long_toolbar(frame);
    }

    fn draw_active_annotation(&self, frame: &mut Image) {
        if !self.is_drawing_annotation {
            return;
        }
        match self.active_tool {
            AnnotationTool::Rectangle => {
                let rect = drag_rect(self.annotation_start, self.annotation_current);
                frame.draw_rect_outline(rect, self.annotation_color, self.annotation_thickness);
            }
            AnnotationTool::Arrow => frame.draw_arrow(&ArrowAnnotation {
                start: self.annotation_start,
                end: self.annotation_current,
                color: self.annotation_color,
                thickness: self.annotation_thickness,
                head_size: 12.0,
            }),
            AnnotationTool::Line => frame.draw_line(
                self.annotation_start,
                self.annotation_current,
                self.annotation_color,
                self.annotation_thickness,
            ),
            AnnotationTool::Freehand => {
                let points = smooth_freehand_points(&self.active_freehand_points);
                frame.draw_polyline(&points, self.annotation_color, self.annotation_thickness);
            }
            _ => {}
        }
    }

    fn toolbar_button_down(&self, button_type: ToolButtonType) -> bool {
        if self.pressed_toolbar_button == Some(button_type) {
            return true;
        }
        matches!(
            (button_type, self.active_tool),
            (ToolButtonType::Rectangle, AnnotationTool::Rectangle)
                | (ToolButtonType::Pen, AnnotationTool::Freehand)
                | (ToolButtonType::Arrow, AnnotationTool::Arrow)
                | (ToolButtonType::Line, AnnotationTool::Line)
        ) || (button_type == ToolButtonType::AutoScroll && self.long_auto_scroll_active)
    }

    fn render_toolbar(&self, frame: &mut Image) {
        frame.fill_round_rect(
            rect_from_f32(
                self.toolbar_x + 2.0,
                self.toolbar_y + 4.0,
                self.toolbar_w,
                self.toolbar_h,
            ),
            CORNER_R,
            TOOLBAR_SHADOW,
        );
        frame.fill_round_rect(
            rect_from_f32(
                self.toolbar_x,
                self.toolbar_y,
                self.toolbar_w,
                self.toolbar_h,
            ),
            CORNER_R,
            TOOLBAR_BG,
        );
        frame.draw_rect_outline(
            rect_from_f32(
                self.toolbar_x,
                self.toolbar_y,
                self.toolbar_w,
                self.toolbar_h,
            ),
            TOOLBAR_STROKE,
            1.0,
        );
        for button in &self.tool_buttons {
            let down = self.toolbar_button_down(button.button_type);
            if down {
                frame.fill_round_rect(rect_from_button(button), BTN_CORNER_R, BTN_DOWN);
            } else if button.hovered {
                frame.fill_round_rect(rect_from_button(button), BTN_CORNER_R, BTN_HOVER);
            }
            draw_button_icon(
                frame,
                button,
                toolbar_icon_color(button.button_type, down),
                self.long_auto_scroll_active,
            );
        }
        self.render_style_panel(frame);
    }

    fn render_style_panel(&self, frame: &mut Image) {
        if self.style_panel_w <= 0.0 || self.style_panel_h <= 0.0 {
            return;
        }

        let label = Rect {
            x: self.style_panel_x.round() as i32,
            y: self.style_panel_y.round() as i32,
            w: STYLE_PANEL_LABEL_W.round() as i32,
            h: STYLE_PANEL_H.round() as i32,
        };
        let panel = Rect {
            x: (self.style_panel_x + STYLE_PANEL_LABEL_W + STYLE_PANEL_GAP).round() as i32,
            y: self.style_panel_y.round() as i32,
            w: STYLE_PANEL_CONTROLS_W.round() as i32,
            h: STYLE_PANEL_H.round() as i32,
        };

        for rect in [label, panel] {
            frame.fill_round_rect(
                Rect {
                    x: rect.x + 2,
                    y: rect.y + 6,
                    w: rect.w,
                    h: rect.h,
                },
                CORNER_R,
                Color::rgba(0, 0, 0, 32),
            );
            frame.fill_round_rect(rect, CORNER_R, TOOLBAR_BG);
            frame.draw_rect_outline(rect, TOOLBAR_STROKE, 1.0);
        }

        draw_text_panel_label(
            frame,
            label.x as f32 + 20.0,
            label.y as f32 + 22.0,
            BTN_ICON,
        );
        draw_style_controls(frame, panel, self.annotation_color);
    }

    fn render_long_toolbar(&self, frame: &mut Image) {
        frame.fill_round_rect(
            rect_from_f32(
                self.toolbar_x + 2.0,
                self.toolbar_y + 4.0,
                self.toolbar_w,
                self.toolbar_h,
            ),
            CORNER_R,
            Color::rgba(0, 0, 0, 42),
        );
        frame.fill_round_rect(
            rect_from_f32(
                self.toolbar_x,
                self.toolbar_y,
                self.toolbar_w,
                self.toolbar_h,
            ),
            CORNER_R,
            LONG_TOOLBAR,
        );
        for button in &self.tool_buttons {
            let down = self.toolbar_button_down(button.button_type);
            if button.hovered || down {
                frame.fill_round_rect(
                    rect_from_f32(
                        button.x + 2.0,
                        button.y + 2.0,
                        button.w - 4.0,
                        button.h - 4.0,
                    ),
                    BTN_CORNER_R,
                    if down { BTN_DOWN } else { BTN_HOVER },
                );
            }
            let icon_color = toolbar_icon_color(button.button_type, down);
            draw_button_icon(frame, button, icon_color, self.long_auto_scroll_active);
        }
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        unsafe {
            HOOK_HWND.store(null_mut(), Ordering::SeqCst);
            if !self.mouse_hook.is_null() {
                let _ = UnhookWindowsHookEx(self.mouse_hook);
                self.mouse_hook = HHOOK::default();
            }
            self.unregister_hotkeys();
            let _ = KillTimer(Some(self.hwnd), TIMER_ID);
            self.destroy_tray();
            if !self.hwnd.is_null() {
                let _ = DestroyWindow(self.hwnd);
                self.hwnd = HWND::default();
            }
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match std::panic::catch_unwind(AssertUnwindSafe(|| unsafe {
        wnd_proc_inner(hwnd, msg, wparam, lparam)
    })) {
        Ok(result) => result,
        Err(_) => {
            append_log(&format!("panic escaped window proc: msg=0x{msg:X}"));
            LRESULT(0)
        }
    }
}

unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_MOUSEWHEEL {
        let hwnd = HWND(HOOK_HWND.load(Ordering::SeqCst));
        if !hwnd.is_null() {
            let mouse = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            let wheel_delta = ((mouse.mouseData >> 16) & 0xffff) as i16 as i32;
            let _ = PostMessageW(
                Some(hwnd),
                WM_LONG_WHEEL,
                WPARAM(wheel_delta as usize),
                make_lparam(mouse.pt.x, mouse.pt.y),
            );
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe fn wnd_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let createstruct = lparam.0 as *const CREATESTRUCTW;
        let app = (*createstruct).lpCreateParams as *mut Application;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as isize);
        (*app).hwnd = hwnd;
    }

    let app = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Application;
    if app.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let app = &mut *app;
    match msg {
        WM_CREATE => LRESULT(0),
        WM_HOTKEY => {
            app.on_hotkey_triggered();
            LRESULT(0)
        }
        WM_TRAYICON => {
            if loword(lparam.0 as usize)
                == crate::windows_api::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP
            {
                app.show_tray_menu();
            } else if loword(lparam.0 as usize)
                == crate::windows_api::Win32::UI::WindowsAndMessaging::WM_LBUTTONDBLCLK
            {
                app.on_hotkey_triggered();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match loword(wparam.0) as usize {
                TRAY_CAPTURE_ID => app.on_hotkey_triggered(),
                TRAY_QUIT_ID => {
                    app.hide_overlay();
                    PostQuitMessage(0);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMER => {
            app.on_timer();
            LRESULT(0)
        }
        WM_PAINT => {
            app.paint();
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            app.on_mouse_down(true, get_x_lparam(lparam), get_y_lparam(lparam));
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            app.on_mouse_down(false, get_x_lparam(lparam), get_y_lparam(lparam));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            app.on_mouse_move(get_x_lparam(lparam), get_y_lparam(lparam));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            app.on_mouse_up(true, get_x_lparam(lparam), get_y_lparam(lparam));
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let mut point = POINT {
                x: get_x_lparam(lparam),
                y: get_y_lparam(lparam),
            };
            let _ = ScreenToClient(hwnd, &mut point);
            app.on_mouse_wheel(get_wheel_delta(wparam), point.x, point.y);
            LRESULT(0)
        }
        WM_LONG_WHEEL => {
            app.handle_native_long_scroll(
                wparam.0 as i16 as i32,
                Point {
                    x: get_x_lparam(lparam),
                    y: get_y_lparam(lparam),
                },
            );
            LRESULT(0)
        }
        WM_KEYDOWN => {
            app.on_key_down(wparam.0 as u32);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn install_panic_logger() {
    std::panic::set_hook(Box::new(|info| {
        append_log(&format!("panic: {info}"));
    }));
}

fn append_log(message: &str) {
    let path = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("screenshot-rust.log"))
        })
        .unwrap_or_else(|| PathBuf::from("screenshot-rust.log"));
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{:?} {}", std::time::SystemTime::now(), message);
    }
}

fn append_long_log(message: &str) {
    if ENABLE_LONG_CAPTURE_LOGS {
        append_log(message);
    }
}

fn render_dim_mask(frame: &mut Image, selected: Rect, color: Color) {
    if selected.w <= 0 || selected.h <= 0 {
        frame.blend_rect(frame.bounds(), color);
        return;
    }
    let bounds = frame.bounds();
    frame.blend_rect(
        Rect {
            x: 0,
            y: 0,
            w: bounds.w,
            h: selected.y,
        },
        color,
    );
    frame.blend_rect(
        Rect {
            x: 0,
            y: selected.bottom(),
            w: bounds.w,
            h: bounds.h - selected.bottom(),
        },
        color,
    );
    frame.blend_rect(
        Rect {
            x: 0,
            y: selected.y,
            w: selected.x,
            h: selected.h,
        },
        color,
    );
    frame.blend_rect(
        Rect {
            x: selected.right(),
            y: selected.y,
            w: bounds.w - selected.right(),
            h: selected.h,
        },
        color,
    );
}

fn draw_selection_frame(frame: &mut Image, rect: Rect, border: Color, accent: Color) {
    frame.draw_rect_outline(rect, border, 2.0);
    frame.draw_line(
        PointF {
            x: rect.x as f32 - 4.0,
            y: rect.y as f32 - 4.0,
        },
        PointF {
            x: rect.x as f32 + 30.0,
            y: rect.y as f32 - 4.0,
        },
        accent,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: rect.x as f32 - 4.0,
            y: rect.y as f32 - 4.0,
        },
        PointF {
            x: rect.x as f32 - 4.0,
            y: rect.y as f32 + 30.0,
        },
        accent,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: rect.right() as f32 + 4.0,
            y: rect.bottom() as f32 + 4.0,
        },
        PointF {
            x: rect.right() as f32 - 30.0,
            y: rect.bottom() as f32 + 4.0,
        },
        accent,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: rect.right() as f32 + 4.0,
            y: rect.bottom() as f32 + 4.0,
        },
        PointF {
            x: rect.right() as f32 + 4.0,
            y: rect.bottom() as f32 - 30.0,
        },
        accent,
        2.0,
    );
}

fn toolbar_icon_color(button_type: ToolButtonType, down: bool) -> Color {
    match button_type {
        ToolButtonType::Cancel => ANNOTATION_RED,
        ToolButtonType::Confirm | ToolButtonType::Ocr => ACTION_SUCCESS,
        ToolButtonType::AutoScroll if down => ANNOTATION_RED,
        _ if down => BTN_ICON_DOWN,
        _ => BTN_ICON,
    }
}

fn draw_button_icon(frame: &mut Image, button: &ToolButton, color: Color, auto_active: bool) {
    let cx = button.x + button.w * 0.5;
    let cy = button.y + button.h * 0.5;
    let pad = if button.w > 38.0 { 10.0 } else { 9.0 };
    match button.button_type {
        ToolButtonType::Rectangle => frame.draw_rect_outline(
            rect_from_f32(
                button.x + pad,
                button.y + pad,
                button.w - pad * 2.0,
                button.h - pad * 2.0,
            ),
            color,
            2.0,
        ),
        ToolButtonType::Circle => frame.draw_ellipse_outline(
            rect_from_f32(
                button.x + pad,
                button.y + pad,
                button.w - pad * 2.0,
                button.h - pad * 2.0,
            ),
            color,
            2.0,
        ),
        ToolButtonType::AddText => draw_icon_a(frame, cx, cy, color),
        ToolButtonType::NumberMarker => draw_icon_number(frame, button, color),
        ToolButtonType::Pen => draw_icon_pen(frame, button, color),
        ToolButtonType::Arrow => frame.draw_arrow(&ArrowAnnotation {
            start: PointF {
                x: button.x + pad,
                y: button.y + button.h - pad,
            },
            end: PointF {
                x: button.x + button.w - pad,
                y: button.y + pad,
            },
            color,
            thickness: 2.0,
            head_size: 8.0,
        }),
        ToolButtonType::Line => frame.draw_line(
            PointF {
                x: button.x + pad,
                y: button.y + button.h - pad,
            },
            PointF {
                x: button.x + button.w - pad,
                y: button.y + pad,
            },
            color,
            2.0,
        ),
        ToolButtonType::DashedLine => draw_dashed_icon_line(frame, button, color),
        ToolButtonType::Mosaic => draw_icon_mosaic(frame, button, color),
        ToolButtonType::Gif => draw_icon_gif(frame, button, color),
        ToolButtonType::Pin => draw_icon_pin(frame, button, color),
        ToolButtonType::Ocr => draw_icon_ocr(frame, button, color),
        ToolButtonType::LongScreenshot => draw_icon_long_screenshot(frame, button, color),
        ToolButtonType::Save => draw_icon_save(frame, button, color),
        ToolButtonType::AutoScroll => draw_icon_auto_scroll(frame, button, color, auto_active),
        ToolButtonType::Edit => draw_icon_edit(frame, button, color),
        ToolButtonType::Undo => draw_icon_undo(frame, button, color),
        ToolButtonType::Cancel => draw_icon_cancel(frame, button, color),
        ToolButtonType::Confirm => draw_icon_confirm(frame, button, color),
    }
}

fn draw_icon_a(frame: &mut Image, cx: f32, cy: f32, color: Color) {
    frame.draw_line(
        PointF {
            x: cx - 10.0,
            y: cy + 10.0,
        },
        PointF {
            x: cx,
            y: cy - 10.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx,
            y: cy - 10.0,
        },
        PointF {
            x: cx + 10.0,
            y: cy + 10.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx - 5.0,
            y: cy + 1.0,
        },
        PointF {
            x: cx + 5.0,
            y: cy + 1.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_number(frame: &mut Image, button: &ToolButton, color: Color) {
    let cx = button.x + button.w * 0.5;
    let cy = button.y + button.h * 0.5;
    frame.draw_ellipse_outline(rect_from_f32(cx - 11.0, cy - 11.0, 22.0, 22.0), color, 2.0);
    frame.draw_line(
        PointF {
            x: cx - 2.5,
            y: cy - 5.5,
        },
        PointF {
            x: cx + 1.5,
            y: cy - 8.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx + 1.5,
            y: cy - 8.0,
        },
        PointF {
            x: cx + 1.5,
            y: cy + 7.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_pen(frame: &mut Image, button: &ToolButton, color: Color) {
    let x = button.x;
    let y = button.y;
    frame.draw_line(
        PointF {
            x: x + 13.0,
            y: y + 34.0,
        },
        PointF {
            x: x + 29.0,
            y: y + 18.0,
        },
        color,
        2.1,
    );
    frame.draw_line(
        PointF {
            x: x + 18.0,
            y: y + 39.0,
        },
        PointF {
            x: x + 34.0,
            y: y + 23.0,
        },
        color,
        2.1,
    );
    frame.draw_line(
        PointF {
            x: x + 29.0,
            y: y + 18.0,
        },
        PointF {
            x: x + 34.0,
            y: y + 13.0,
        },
        color,
        2.1,
    );
    frame.draw_line(
        PointF {
            x: x + 34.0,
            y: y + 13.0,
        },
        PointF {
            x: x + 39.0,
            y: y + 18.0,
        },
        color,
        2.1,
    );
    frame.draw_line(
        PointF {
            x: x + 39.0,
            y: y + 18.0,
        },
        PointF {
            x: x + 34.0,
            y: y + 23.0,
        },
        color,
        2.1,
    );
    frame.draw_line(
        PointF {
            x: x + 13.0,
            y: y + 34.0,
        },
        PointF {
            x: x + 10.0,
            y: y + 42.0,
        },
        color,
        2.1,
    );
    frame.draw_line(
        PointF {
            x: x + 10.0,
            y: y + 42.0,
        },
        PointF {
            x: x + 18.0,
            y: y + 39.0,
        },
        color,
        2.1,
    );
}

fn draw_dashed_icon_line(frame: &mut Image, button: &ToolButton, color: Color) {
    let start = PointF {
        x: button.x + 9.0,
        y: button.y + button.h - 9.0,
    };
    let end = PointF {
        x: button.x + button.w - 9.0,
        y: button.y + 9.0,
    };
    draw_dashed_line(frame, start, end, color, 2.4, 4.0, 5.0);
}

fn draw_icon_mosaic(frame: &mut Image, button: &ToolButton, color: Color) {
    let x = button.x + 10.0;
    let y = button.y + 9.0;
    let cell = 8.0;
    frame.draw_rect_outline(rect_from_f32(x, y, cell, cell), color, 1.8);
    frame.blend_rect(rect_from_f32(x + cell + 2.0, y, cell, cell), color);
    frame.blend_rect(rect_from_f32(x, y + cell + 2.0, cell, cell), color);
    frame.draw_rect_outline(
        rect_from_f32(x + cell + 2.0, y + cell + 2.0, cell, cell),
        color,
        1.8,
    );
}

fn draw_icon_gif(frame: &mut Image, button: &ToolButton, color: Color) {
    frame.draw_rect_outline(
        rect_from_f32(
            button.x + 7.5,
            button.y + 9.0,
            button.w - 15.0,
            button.h - 18.0,
        ),
        color,
        1.7,
    );
    let top = button.y + 15.0;
    draw_glyph_g(frame, button.x + 11.0, top, color);
    draw_glyph_i(frame, button.x + 19.0, top, color);
    draw_glyph_f(frame, button.x + 25.0, top, color);
}

fn draw_icon_pin(frame: &mut Image, button: &ToolButton, color: Color) {
    let cx = button.x + button.w * 0.5;
    let cy = button.y + button.h * 0.5;
    frame.draw_line(
        PointF {
            x: cx - 6.0,
            y: cy - 8.0,
        },
        PointF {
            x: cx + 7.0,
            y: cy + 5.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx - 10.0,
            y: cy - 2.0,
        },
        PointF {
            x: cx,
            y: cy - 12.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx + 1.0,
            y: cy + 4.0,
        },
        PointF {
            x: cx - 7.0,
            y: cy + 12.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx - 8.0,
            y: cy - 6.0,
        },
        PointF {
            x: cx - 4.0,
            y: cy - 10.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_ocr(frame: &mut Image, button: &ToolButton, color: Color) {
    let left = button.x + 9.0;
    let top = button.y + 10.0;
    let right = button.x + button.w - 9.0;
    let bottom = button.y + button.h - 10.0;
    let corner = 5.0;
    for (x, y, sx, sy) in [
        (left, top, 1.0, 1.0),
        (right, top, -1.0, 1.0),
        (left, bottom, 1.0, -1.0),
        (right, bottom, -1.0, -1.0),
    ] {
        frame.draw_line(
            PointF { x, y },
            PointF {
                x: x + sx * corner,
                y,
            },
            color,
            2.0,
        );
        frame.draw_line(
            PointF { x, y },
            PointF {
                x,
                y: y + sy * corner,
            },
            color,
            2.0,
        );
    }
    frame.draw_line(
        PointF {
            x: button.x + 15.0,
            y: button.y + 14.0,
        },
        PointF {
            x: button.x + 23.0,
            y: button.y + 14.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: button.x + 19.0,
            y: button.y + 14.0,
        },
        PointF {
            x: button.x + 19.0,
            y: button.y + 25.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: button.x + 28.0,
            y: button.y + 10.0,
        },
        PointF {
            x: button.x + 28.0,
            y: button.y + 16.0,
        },
        color,
        1.6,
    );
    frame.draw_line(
        PointF {
            x: button.x + 25.0,
            y: button.y + 13.0,
        },
        PointF {
            x: button.x + 31.0,
            y: button.y + 13.0,
        },
        color,
        1.6,
    );
}

fn draw_icon_long_screenshot(frame: &mut Image, button: &ToolButton, color: Color) {
    let x = button.x + 11.0;
    let y = button.y + 7.0;
    frame.draw_rect_outline(rect_from_f32(x, y, 16.0, 24.0), color, 1.8);
    frame.draw_line(
        PointF {
            x: x + 4.0,
            y: y + 6.0,
        },
        PointF {
            x: x + 12.0,
            y: y + 6.0,
        },
        color,
        1.6,
    );
    frame.draw_line(
        PointF {
            x: x + 4.0,
            y: y + 12.0,
        },
        PointF {
            x: x + 12.0,
            y: y + 12.0,
        },
        color,
        1.6,
    );
    frame.draw_line(
        PointF {
            x: x + 8.0,
            y: y + 15.0,
        },
        PointF {
            x: x + 8.0,
            y: y + 25.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: x + 8.0,
            y: y + 25.0,
        },
        PointF {
            x: x + 4.0,
            y: y + 21.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: x + 8.0,
            y: y + 25.0,
        },
        PointF {
            x: x + 12.0,
            y: y + 21.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_save(frame: &mut Image, button: &ToolButton, color: Color) {
    let x = button.x + 10.0;
    let y = button.y + 8.0;
    frame.draw_rect_outline(rect_from_f32(x, y, 18.0, 22.0), color, 1.9);
    frame.draw_line(
        PointF { x: x + 4.0, y },
        PointF {
            x: x + 4.0,
            y: y + 7.0,
        },
        color,
        1.8,
    );
    frame.draw_line(
        PointF { x: x + 13.0, y },
        PointF {
            x: x + 13.0,
            y: y + 7.0,
        },
        color,
        1.8,
    );
    frame.draw_rect_outline(rect_from_f32(x + 5.0, y + 13.0, 8.0, 7.0), color, 1.6);
}

fn draw_icon_auto_scroll(frame: &mut Image, button: &ToolButton, color: Color, auto_active: bool) {
    let cx = button.x + button.w * 0.5;
    let cy = button.y + button.h * 0.5;
    if auto_active {
        frame.blend_rect(rect_from_f32(cx - 5.0, cy - 6.0, 10.0, 12.0), color);
        return;
    }
    frame.draw_line(
        PointF {
            x: cx - 6.0,
            y: cy - 9.0,
        },
        PointF {
            x: cx - 6.0,
            y: cy + 9.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx - 6.0,
            y: cy - 9.0,
        },
        PointF { x: cx + 9.0, y: cy },
        color,
        2.0,
    );
    frame.draw_line(
        PointF { x: cx + 9.0, y: cy },
        PointF {
            x: cx - 6.0,
            y: cy + 9.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_edit(frame: &mut Image, button: &ToolButton, color: Color) {
    frame.draw_rect_outline(
        rect_from_f32(
            button.x + 10.0,
            button.y + 10.0,
            button.w - 22.0,
            button.h - 21.0,
        ),
        color,
        1.6,
    );
    frame.draw_line(
        PointF {
            x: button.x + 18.0,
            y: button.y + 27.0,
        },
        PointF {
            x: button.x + 28.0,
            y: button.y + 17.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_undo(frame: &mut Image, button: &ToolButton, color: Color) {
    let cx = button.x + button.w * 0.5;
    let cy = button.y + button.h * 0.5;
    let pts = [
        PointF {
            x: cx + 9.0,
            y: cy + 7.0,
        },
        PointF {
            x: cx + 4.0,
            y: cy - 4.0,
        },
        PointF {
            x: cx - 7.0,
            y: cy - 5.0,
        },
        PointF {
            x: cx - 10.0,
            y: cy,
        },
    ];
    frame.draw_polyline(&pts, color, 2.0);
    frame.draw_line(
        PointF {
            x: cx - 10.0,
            y: cy,
        },
        PointF {
            x: cx - 4.0,
            y: cy - 7.0,
        },
        color,
        2.0,
    );
    frame.draw_line(
        PointF {
            x: cx - 10.0,
            y: cy,
        },
        PointF {
            x: cx - 3.0,
            y: cy + 5.0,
        },
        color,
        2.0,
    );
}

fn draw_icon_cancel(frame: &mut Image, button: &ToolButton, color: Color) {
    frame.draw_line(
        PointF {
            x: button.x + 10.0,
            y: button.y + 10.0,
        },
        PointF {
            x: button.x + button.w - 10.0,
            y: button.y + button.h - 10.0,
        },
        color,
        2.4,
    );
    frame.draw_line(
        PointF {
            x: button.x + button.w - 10.0,
            y: button.y + 10.0,
        },
        PointF {
            x: button.x + 10.0,
            y: button.y + button.h - 10.0,
        },
        color,
        2.4,
    );
}

fn draw_icon_confirm(frame: &mut Image, button: &ToolButton, color: Color) {
    let cx = button.x + button.w * 0.5;
    let cy = button.y + button.h * 0.5;
    frame.draw_line(
        PointF {
            x: cx - 10.0,
            y: cy + 1.0,
        },
        PointF {
            x: cx - 3.0,
            y: cy + 8.0,
        },
        color,
        2.4,
    );
    frame.draw_line(
        PointF {
            x: cx - 3.0,
            y: cy + 8.0,
        },
        PointF {
            x: cx + 11.0,
            y: cy - 9.0,
        },
        color,
        2.4,
    );
}

fn draw_text_panel_label(frame: &mut Image, x: f32, y: f32, color: Color) {
    let thick = 2.1;

    let wx = x;
    let wy = y;
    frame.draw_line(
        PointF {
            x: wx + 14.0,
            y: wy,
        },
        PointF {
            x: wx + 14.0,
            y: wy + 5.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: wx + 3.0,
            y: wy + 9.0,
        },
        PointF {
            x: wx + 25.0,
            y: wy + 9.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: wx + 7.0,
            y: wy + 15.0,
        },
        PointF {
            x: wx + 23.0,
            y: wy + 31.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: wx + 22.0,
            y: wy + 15.0,
        },
        PointF {
            x: wx + 4.0,
            y: wy + 31.0,
        },
        color,
        thick,
    );

    let bx = x + 34.0;
    let by = y;
    frame.draw_line(
        PointF {
            x: bx + 14.0,
            y: by,
        },
        PointF {
            x: bx + 14.0,
            y: by + 32.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: bx + 3.0,
            y: by + 8.0,
        },
        PointF {
            x: bx + 25.0,
            y: by + 8.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: bx + 5.0,
            y: by + 20.0,
        },
        PointF {
            x: bx + 23.0,
            y: by + 20.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: bx + 14.0,
            y: by + 12.0,
        },
        PointF {
            x: bx + 4.0,
            y: by + 30.0,
        },
        color,
        thick,
    );
    frame.draw_line(
        PointF {
            x: bx + 14.0,
            y: by + 12.0,
        },
        PointF {
            x: bx + 25.0,
            y: by + 30.0,
        },
        color,
        thick,
    );
}

fn draw_style_controls(frame: &mut Image, panel: Rect, active_color: Color) {
    let input = Rect {
        x: panel.x + STYLE_PANEL_PAD.round() as i32 + 10,
        y: panel.y + 12,
        w: 132,
        h: 54,
    };
    frame.fill_round_rect(input, 6.0, Color::rgba(255, 255, 255, 255));
    frame.draw_rect_outline(input, Color::rgba(218, 222, 228, 210), 1.0);
    draw_segment_text(
        frame,
        "5",
        input.x + 34,
        input.y + 16,
        4,
        Color::rgba(19, 19, 19, 255),
    );
    draw_stepper_arrow(
        frame,
        input.x as f32 + input.w as f32 - 40.0,
        input.y as f32 + 18.0,
        true,
        BTN_ICON,
    );
    draw_stepper_arrow(
        frame,
        input.x as f32 + input.w as f32 - 40.0,
        input.y as f32 + 36.0,
        false,
        BTN_ICON,
    );

    let colors = [
        Color::rgba(0, 0, 0, 255),
        Color::rgba(61, 61, 61, 255),
        Color::rgba(255, 255, 255, 255),
        ANNOTATION_RED,
        Color::rgba(255, 171, 22, 255),
        Color::rgba(0, 179, 35, 255),
        Color::rgba(24, 168, 181, 255),
        Color::rgba(27, 68, 238, 255),
        Color::rgba(69, 38, 184, 255),
        Color::rgba(198, 21, 203, 255),
    ];
    let swatch = 48;
    let gap = 22;
    let mut x = input.x + input.w + STYLE_PANEL_SECTION_GAP.round() as i32 + 10;
    let y = panel.y + 15;
    for color in colors {
        let rect = Rect {
            x,
            y,
            w: swatch,
            h: swatch,
        };
        frame.fill_round_rect(rect, 4.0, color);
        let outline = if color == Color::rgba(255, 255, 255, 255) {
            Color::rgba(205, 210, 218, 255)
        } else {
            Color::rgba(0, 0, 0, 30)
        };
        frame.draw_rect_outline(rect, outline, 1.0);
        if color == active_color {
            draw_swatch_check(frame, rect, Color::rgba(255, 255, 255, 255));
        }
        x += swatch + gap;
    }
}

fn draw_stepper_arrow(frame: &mut Image, cx: f32, cy: f32, up: bool, color: Color) {
    let y = if up { cy + 4.0 } else { cy - 4.0 };
    frame.draw_line(
        PointF { x: cx - 9.0, y },
        PointF { x: cx, y: cy },
        color,
        2.2,
    );
    frame.draw_line(
        PointF { x: cx, y: cy },
        PointF { x: cx + 9.0, y },
        color,
        2.2,
    );
}

fn draw_swatch_check(frame: &mut Image, rect: Rect, color: Color) {
    let cx = rect.x as f32 + rect.w as f32 * 0.5;
    let cy = rect.y as f32 + rect.h as f32 * 0.5;
    frame.draw_line(
        PointF {
            x: cx - 11.0,
            y: cy + 1.0,
        },
        PointF {
            x: cx - 4.0,
            y: cy + 8.0,
        },
        color,
        2.8,
    );
    frame.draw_line(
        PointF {
            x: cx - 4.0,
            y: cy + 8.0,
        },
        PointF {
            x: cx + 12.0,
            y: cy - 11.0,
        },
        color,
        2.8,
    );
}

fn draw_dashed_line(
    frame: &mut Image,
    start: PointF,
    end: PointF,
    color: Color,
    thickness: f32,
    dash: f32,
    gap: f32,
) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.1 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let mut offset = 0.0;
    while offset < len {
        let next = (offset + dash).min(len);
        frame.draw_line(
            PointF {
                x: start.x + ux * offset,
                y: start.y + uy * offset,
            },
            PointF {
                x: start.x + ux * next,
                y: start.y + uy * next,
            },
            color,
            thickness,
        );
        offset += dash + gap;
    }
}

fn draw_glyph_g(frame: &mut Image, x: f32, y: f32, color: Color) {
    frame.draw_line(PointF { x, y }, PointF { x: x + 5.0, y }, color, 1.4);
    frame.draw_line(PointF { x, y }, PointF { x, y: y + 9.0 }, color, 1.4);
    frame.draw_line(
        PointF { x, y: y + 9.0 },
        PointF {
            x: x + 5.0,
            y: y + 9.0,
        },
        color,
        1.4,
    );
    frame.draw_line(
        PointF {
            x: x + 5.0,
            y: y + 5.0,
        },
        PointF {
            x: x + 5.0,
            y: y + 9.0,
        },
        color,
        1.4,
    );
    frame.draw_line(
        PointF {
            x: x + 2.5,
            y: y + 5.0,
        },
        PointF {
            x: x + 5.0,
            y: y + 5.0,
        },
        color,
        1.4,
    );
}

fn draw_glyph_i(frame: &mut Image, x: f32, y: f32, color: Color) {
    frame.draw_line(PointF { x, y }, PointF { x: x + 5.0, y }, color, 1.4);
    frame.draw_line(
        PointF { x: x + 2.5, y },
        PointF {
            x: x + 2.5,
            y: y + 9.0,
        },
        color,
        1.4,
    );
    frame.draw_line(
        PointF { x, y: y + 9.0 },
        PointF {
            x: x + 5.0,
            y: y + 9.0,
        },
        color,
        1.4,
    );
}

fn draw_glyph_f(frame: &mut Image, x: f32, y: f32, color: Color) {
    frame.draw_line(PointF { x, y }, PointF { x: x + 5.0, y }, color, 1.4);
    frame.draw_line(PointF { x, y }, PointF { x, y: y + 9.0 }, color, 1.4);
    frame.draw_line(
        PointF { x, y: y + 4.5 },
        PointF {
            x: x + 4.0,
            y: y + 4.5,
        },
        color,
        1.4,
    );
}

fn render_dimension_badge(frame: &mut Image, rect: Rect) {
    if rect.w <= 0 || rect.h <= 0 {
        return;
    }
    let text = format!("{}*{}", rect.w, rect.h);
    let scale = 3;
    let pad_x = 12;
    let pad_y = 6;
    let text_w = measure_segment_text(&text, scale);
    let text_h = 7 * scale;
    let badge_w = text_w + pad_x * 2;
    let badge_h = text_h + pad_y * 2;
    let bounds = frame.bounds();
    let x = rect.x.clamp(0, (bounds.w - badge_w).max(0));
    let above_y = rect.y - badge_h - 4;
    let y = if above_y >= 0 {
        above_y
    } else {
        (rect.y + 4).clamp(0, (bounds.h - badge_h).max(0))
    };
    frame.fill_round_rect(
        Rect {
            x,
            y,
            w: badge_w,
            h: badge_h,
        },
        6.0,
        SEL_BORDER,
    );
    draw_segment_text(frame, &text, x + pad_x, y + pad_y, scale, BADGE_TEXT);
}

fn measure_segment_text(text: &str, scale: i32) -> i32 {
    let char_w = 5 * scale;
    let gap = 2 * scale;
    let count = text.chars().count() as i32;
    if count == 0 {
        0
    } else {
        count * char_w + (count - 1) * gap
    }
}

fn draw_segment_text(frame: &mut Image, text: &str, x: i32, y: i32, scale: i32, color: Color) {
    let char_w = 5 * scale;
    let gap = 2 * scale;
    let mut cx = x;
    for ch in text.chars() {
        draw_segment_char(frame, ch, cx, y, scale, color);
        cx += char_w + gap;
    }
}

fn draw_segment_char(frame: &mut Image, ch: char, x: i32, y: i32, scale: i32, color: Color) {
    if ch == '*' {
        let cx = x as f32 + 2.5 * scale as f32;
        let cy = y as f32 + 3.5 * scale as f32;
        let r = 2.0 * scale as f32;
        frame.draw_line(
            PointF {
                x: cx - r,
                y: cy - r,
            },
            PointF {
                x: cx + r,
                y: cy + r,
            },
            color,
            scale as f32,
        );
        frame.draw_line(
            PointF {
                x: cx + r,
                y: cy - r,
            },
            PointF {
                x: cx - r,
                y: cy + r,
            },
            color,
            scale as f32,
        );
        return;
    }

    let Some(segments) = digit_segments(ch) else {
        return;
    };
    for segment in segments {
        draw_segment(frame, segment, x, y, scale, color);
    }
}

fn digit_segments(ch: char) -> Option<&'static [u8]> {
    match ch {
        '0' => Some(&[0, 1, 2, 3, 4, 5]),
        '1' => Some(&[1, 2]),
        '2' => Some(&[0, 1, 6, 4, 3]),
        '3' => Some(&[0, 1, 6, 2, 3]),
        '4' => Some(&[5, 6, 1, 2]),
        '5' => Some(&[0, 5, 6, 2, 3]),
        '6' => Some(&[0, 5, 6, 4, 2, 3]),
        '7' => Some(&[0, 1, 2]),
        '8' => Some(&[0, 1, 2, 3, 4, 5, 6]),
        '9' => Some(&[0, 1, 2, 3, 5, 6]),
        _ => None,
    }
}

fn draw_segment(frame: &mut Image, segment: &u8, x: i32, y: i32, scale: i32, color: Color) {
    let s = scale as f32;
    let xf = x as f32;
    let yf = y as f32;
    let (start, end) = match *segment {
        0 => ((1.0, 0.0), (4.0, 0.0)),
        1 => ((4.0, 0.0), (4.0, 3.0)),
        2 => ((4.0, 3.0), (4.0, 6.0)),
        3 => ((1.0, 6.0), (4.0, 6.0)),
        4 => ((0.0, 3.0), (0.0, 6.0)),
        5 => ((0.0, 0.0), (0.0, 3.0)),
        6 => ((1.0, 3.0), (4.0, 3.0)),
        _ => return,
    };
    frame.draw_line(
        PointF {
            x: xf + start.0 * s,
            y: yf + start.1 * s,
        },
        PointF {
            x: xf + end.0 * s,
            y: yf + end.1 * s,
        },
        color,
        s,
    );
}

fn capture_covered_window(desktop_bounds: Rect, overlay_hwnd: HWND, region: Rect) -> Option<Image> {
    if region.w <= 0 || region.h <= 0 {
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
        let mut bits: *mut c_void = null_mut();
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
            image = Some(Image {
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

fn capture_region(desktop_bounds: Rect, region: Rect) -> Option<Image> {
    if region.w <= 0 || region.h <= 0 {
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
        let mut bits: *mut c_void = null_mut();
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
        ok.then_some(Image {
            width: region.w,
            height: region.h,
            pixels: rgba,
        })
    }
}

unsafe fn present_image(hdc: HDC, image: &mut Image) {
    let mut bgra = vec![0; image.pixels.len()];
    for (src, dst) in image.pixels.chunks_exact(4).zip(bgra.chunks_exact_mut(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 255;
    }
    let mut bmi: BITMAPINFO = zeroed();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: image.width,
        biHeight: -image.height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: (image.width * image.height * 4) as u32,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    SetDIBitsToDevice(
        hdc,
        0,
        0,
        image.width as u32,
        image.height as u32,
        0,
        0,
        0,
        image.height as u32,
        bgra.as_ptr().cast(),
        &bmi,
        DIB_RGB_COLORS,
    );
}

unsafe fn write_clipboard_image(hwnd: HWND, rgba: &[u8], width: i32, height: i32) -> bool {
    if OpenClipboard(Some(hwnd)).is_err() {
        return false;
    }
    let _ = EmptyClipboard();
    let header_size = size_of::<BITMAPINFOHEADER>();
    let pixel_size = width as usize * height as usize * 4;
    let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, header_size + pixel_size) else {
        let _ = CloseClipboard();
        return false;
    };
    let ptr = GlobalLock(hmem) as *mut u8;
    if ptr.is_null() {
        let _ = CloseClipboard();
        return false;
    }
    let bih = ptr as *mut BITMAPINFOHEADER;
    *bih = BITMAPINFOHEADER {
        biSize: header_size as u32,
        biWidth: width,
        biHeight: height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: pixel_size as u32,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };
    let dst_pixels = std::slice::from_raw_parts_mut(ptr.add(header_size), pixel_size);
    for y in 0..height as usize {
        let src_row = (height as usize - 1 - y) * width as usize * 4;
        let dst_row = y * width as usize * 4;
        for x in 0..width as usize {
            let src = src_row + x * 4;
            let dst = dst_row + x * 4;
            dst_pixels[dst] = rgba[src + 2];
            dst_pixels[dst + 1] = rgba[src + 1];
            dst_pixels[dst + 2] = rgba[src];
            dst_pixels[dst + 3] = rgba[src + 3];
        }
    }
    let _ = GlobalUnlock(hmem);
    let _ = SetClipboardData(CF_DIB_FORMAT, Some(HANDLE(hmem.0)));

    if let Some(png) = encode_rgba_png(width, height, rgba) {
        let format_name = to_wide("PNG");
        let png_format = RegisterClipboardFormatW(pcwstr(&format_name));
        if png_format != 0 {
            if let Ok(png_mem) = GlobalAlloc(GMEM_MOVEABLE, png.len()) {
                let png_ptr = GlobalLock(png_mem) as *mut u8;
                if !png_ptr.is_null() {
                    std::ptr::copy_nonoverlapping(png.as_ptr(), png_ptr, png.len());
                    let _ = GlobalUnlock(png_mem);
                    let _ = SetClipboardData(png_format, Some(HANDLE(png_mem.0)));
                }
            }
        }
    }

    let _ = CloseClipboard();
    true
}

unsafe fn show_save_dialog(hwnd: HWND) -> Option<PathBuf> {
    show_save_dialog_ext(hwnd, "PNG Files (*.png)", "*.png", "png", "screenshot.png")
}

unsafe fn show_gif_save_dialog(hwnd: HWND) -> Option<PathBuf> {
    show_save_dialog_ext(hwnd, "GIF Files (*.gif)", "*.gif", "gif", "recording.gif")
}

unsafe fn show_save_dialog_ext(
    hwnd: HWND,
    file_label: &str,
    file_pattern: &str,
    default_ext: &str,
    default_file: &str,
) -> Option<PathBuf> {
    let mut filename = [0_u16; 260];
    let default_name = to_wide(default_file);
    for (dst, src) in filename.iter_mut().zip(default_name.iter()) {
        *dst = *src;
    }
    let filter: Vec<u16> = OsStr::new(file_label)
        .encode_wide()
        .chain(Some(0))
        .chain(OsStr::new(file_pattern).encode_wide())
        .chain(Some(0))
        .chain(OsStr::new("All Files (*.*)").encode_wide())
        .chain(Some(0))
        .chain(OsStr::new("*.*").encode_wide())
        .chain(Some(0))
        .chain(Some(0))
        .collect();
    let mut ofn: OPENFILENAMEW = zeroed();
    let def_ext = to_wide(default_ext);
    ofn.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = hwnd;
    ofn.lpstrFilter = pcwstr(&filter);
    ofn.lpstrFile = pwstr(&mut filename);
    ofn.nMaxFile = filename.len() as u32;
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;
    ofn.lpstrDefExt = pcwstr(&def_ext);
    if GetSaveFileNameW(&mut ofn).0 == 0 {
        return None;
    }
    let len = filename
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(filename.len());
    Some(PathBuf::from(OsString::from_wide(&filename[..len])))
}

fn virtual_desktop_bounds() -> Rect {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        Rect {
            x,
            y,
            w: if w > 0 { w } else { 1920 },
            h: if h > 0 { h } else { 1080 },
        }
    }
}

fn drag_rect(a: PointF, b: PointF) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    Rect {
        x: x0.round() as i32,
        y: y0.round() as i32,
        w: (x1 - x0).round() as i32,
        h: (y1 - y0).round() as i32,
    }
}

fn rect_from_f32(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect {
        x: x.round() as i32,
        y: y.round() as i32,
        w: w.round() as i32,
        h: h.round() as i32,
    }
}

fn rect_from_button(button: &ToolButton) -> Rect {
    rect_from_f32(button.x, button.y, button.w, button.h)
}

fn point_in_button(x: f32, y: f32, button: &ToolButton) -> bool {
    x >= button.x && x <= button.x + button.w && y >= button.y && y <= button.y + button.h
}

fn padded_rect(rect: Rect, pad: i32) -> Rect {
    Rect {
        x: rect.x - pad,
        y: rect.y - pad,
        w: rect.w + pad * 2,
        h: rect.h + pad * 2,
    }
}

unsafe fn add_rect_to_region(region: HRGN, size: Size, x: i32, y: i32, w: i32, h: i32) {
    let left = x.clamp(0, size.w);
    let top = y.clamp(0, size.h);
    let right = (x + w).clamp(0, size.w);
    let bottom = (y + h).clamp(0, size.h);
    if right <= left || bottom <= top {
        return;
    }
    let rect_region = CreateRectRgn(left, top, right, bottom);
    if rect_region.is_null() {
        return;
    }
    CombineRgn(Some(region), Some(region), Some(rect_region), RGN_OR);
    let _ = DeleteObject(HGDIOBJ::from(rect_region));
}

fn push_toolbar_button(
    buttons: &mut Vec<ToolButton>,
    bx: &mut f32,
    y: f32,
    button_type: ToolButtonType,
) {
    buttons.push(ToolButton {
        x: *bx,
        y,
        w: BTN_SIZE,
        h: BTN_SIZE,
        button_type,
        hovered: false,
    });
    *bx += BTN_SIZE + BTN_GAP;
}

fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn copy_wide_into(dst: &mut [u16], src: &[u16]) {
    let len = dst.len().min(src.len());
    dst[..len].copy_from_slice(&src[..len]);
    if let Some(last) = dst.last_mut() {
        *last = 0;
    }
}

fn get_x_lparam(lparam: LPARAM) -> i32 {
    (lparam.0 as u32 & 0xffff) as i16 as i32
}

fn max_instant(a: Instant, b: Instant) -> Instant {
    if a >= b {
        a
    } else {
        b
    }
}

fn get_y_lparam(lparam: LPARAM) -> i32 {
    ((lparam.0 as u32 >> 16) & 0xffff) as i16 as i32
}

fn get_wheel_delta(wparam: WPARAM) -> i32 {
    ((wparam.0 >> 16) & 0xffff) as i16 as i32
}

fn loword(value: usize) -> u32 {
    (value & 0xffff) as u32
}

fn make_lparam(x: i32, y: i32) -> LPARAM {
    LPARAM(((x as u16 as usize) | ((y as u16 as usize) << 16)) as isize)
}
