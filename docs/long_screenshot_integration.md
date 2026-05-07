# 长截屏接入指南

这份文档给你接入长截屏用。你不用自己写截屏、滚轮转发、窗口查找、后台捕获和拼接逻辑；这些都已经包在 `src/longshot/` 里了。

你要做的事情只有三件：

1. 复制 `src/longshot/`
2. 在点击“长截屏”按钮时调用 `LongShotRuntime::start(...)`
3. 在窗口 timer 和鼠标滚轮消息里把事件转给 `LongShotRuntime`

最终导出时调用 `finish_for_export(...)`，拿到的就是完整长图 RGBA。

## 1. 复制文件

把整个目录复制过去：

```text
src/longshot/
  mod.rs
  runtime.rs
  session.rs
  stitcher.rs
  types.rs
  windows.rs
```

然后在 `lib.rs` 或 `main.rs` 里声明：

```rust
pub mod longshot;
```

`LongShotRuntime` 是你优先使用的入口。其他文件只是它内部用的拆分模块。

## 2. 依赖

`LongShotRuntime` 会使用 `windows.rs` 里的 Win32 捕获和滚轮转发，所以 `Cargo.toml` 里需要这些 feature：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_Storage_Xps",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
] }
```

本项目里为了兼容旧代码写的是：

```rust
#[cfg(windows)]
extern crate windows as windows_api;
```

所以 `src/longshot/windows.rs` 里引用的是：

```rust
crate::windows_api::...
```

如果你的工程里没有 `windows_api` 这个别名，二选一：

```rust
#[cfg(windows)]
extern crate windows as windows_api;
```

或者把 `src/longshot/windows.rs` 里的 `crate::windows_api::` 全部替换成 `windows::`。

## 3. 一句话理解接口

你只跟这个对象打交道：

```rust
use crate::longshot::{
    LongShotPoint, LongShotRect, LongShotRuntime, LongShotRuntimeEvent,
};

struct AppState {
    longshot: LongShotRuntime,
}
```

初始化：

```rust
let longshot = LongShotRuntime::default();
```

点击长截屏按钮时：

```rust
let event = self.longshot.start(
    desktop_bounds,
    self.hwnd.0 as isize,
    selected_region,
)?;
```

鼠标滚轮时：

```rust
self.longshot.handle_manual_wheel(screen_point, wheel_delta);
```

窗口 timer 时：

```rust
for event in self.longshot.tick() {
    self.handle_longshot_event(event);
}
```

保存或复制时：

```rust
let image = self.longshot.finish_for_export(Duration::from_millis(500));
```

## 4. 数据格式

长截屏输出是 RGBA、top-down：

```rust
LongShotImage {
    width: i32,
    height: i32,
    pixels: Vec<u8>, // RGBA, width * height * 4
}
```

选区和桌面范围都用：

```rust
LongShotRect { x, y, w, h }
```

注意：`selected_region` 是相对虚拟桌面的坐标。多屏时虚拟桌面左上角可能不是 `(0, 0)`。

你可以这样取虚拟桌面范围：

```rust
fn virtual_desktop_bounds() -> LongShotRect {
    unsafe {
        LongShotRect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            w: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            h: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}
```

## 5. 点击长截屏按钮

当你的普通截图流程已经让用户框选出一个区域后，在“长截屏”按钮点击处调用：

```rust
fn start_longshot(&mut self) -> bool {
    let desktop_bounds = self.virtual_desktop_bounds();
    let selected = self.current_selected_region();
    let selected = LongShotRect::new(selected.x, selected.y, selected.w, selected.h);

    let event = match self.longshot.start(
        desktop_bounds,
        self.hwnd.0 as isize,
        selected,
    ) {
        Ok(event) => event,
        Err(_) => return false,
    };

    self.apply_longshot_event(event);

    // 建议：长截屏期间让选区内部鼠标穿透。
    // Runtime 会自己找到浮层下面的真实窗口并投递滚轮。
    self.set_passthrough_region(selected);

    true
}
```

`start(...)` 会自己做第一帧捕获，不需要你提前把全屏截图传进来。

## 6. 处理 Runtime 事件

Runtime 会通过事件告诉你什么时候刷新预览、什么时候停止自动滚动：

```rust
fn apply_longshot_event(&mut self, event: LongShotRuntimeEvent) {
    match event {
        LongShotRuntimeEvent::Started { image } |
        LongShotRuntimeEvent::PreviewUpdated { image, .. } => {
            self.show_longshot_preview(image);
        }
        LongShotRuntimeEvent::AutoScrollStopped |
        LongShotRuntimeEvent::MaxOutputHeightReached => {
            self.update_longshot_toolbar_state();
        }
        LongShotRuntimeEvent::FrameIgnored { .. } |
        LongShotRuntimeEvent::CaptureFailed { .. } => {}
    }
}
```

`show_longshot_preview(image)` 由你的 UI 实现：把长图缩放显示到选区附近，或者直接替换当前预览图都可以。

## 7. 鼠标滚轮

收到 `WM_MOUSEWHEEL` 时，把屏幕坐标和滚轮值交给 Runtime：

```rust
fn on_mouse_wheel(&mut self, wheel_delta: i32, screen_x: i32, screen_y: i32) {
    let screen_point = LongShotPoint {
        x: screen_x,
        y: screen_y,
    };

    if self.longshot.handle_manual_wheel(screen_point, wheel_delta) {
        self.ensure_timer_running();
    }
}
```

`handle_manual_wheel(...)` 内部会做这些事：

- 找到截图浮层下面的真实窗口
- 投递 `WM_MOUSEWHEEL`
- 延迟约 50ms 后安排下一帧捕获
- 后台捕获选区
- 调拼接算法
- 在 `tick()` 里返回预览更新事件

你不要再额外写一套滚轮转发。

## 8. Timer

长截屏开始后，保持一个短周期 timer，例如 15-30ms：

```rust
fn on_timer(&mut self) {
    for event in self.longshot.tick() {
        self.apply_longshot_event(event);
    }
}
```

`tick()` 会自动处理：

- 后台捕获线程结果
- 手动滚轮后的延迟捕获
- 自动滚动
- 连续无新增内容后停止
- 输出高度上限

## 9. 自动滚动

自动滚动按钮只需要切 Runtime 状态：

```rust
fn toggle_longshot_auto_scroll(&mut self) {
    let active = self.longshot.toggle_auto_scroll();
    self.set_auto_scroll_button_active(active);
    self.ensure_timer_running();
}
```

或者明确开启/关闭：

```rust
self.longshot.set_auto_scroll(true);
self.longshot.set_auto_scroll(false);
```

自动滚动内部使用 `windows.rs` 投递滚轮，默认每 120ms 滚一次，每次约 `-40` wheel delta。连续 5 次没有拼接出新内容会自动停止。

## 10. 保存和复制

保存、复制、贴图前调用：

```rust
fn export_longshot(&mut self) -> Option<LongShotImage> {
    self.longshot.finish_for_export(Duration::from_millis(500))
}
```

`finish_for_export(...)` 会：

- 关闭自动滚动
- 等待正在进行的后台捕获
- 再补抓一次当前选区
- 返回最终长图

导出 PNG 或写剪贴板时直接用返回的 RGBA：

```rust
let image = self.export_longshot()?;
save_png_or_copy_clipboard(image.pixels, image.width, image.height);
```

如果你有标注功能，长截图导出时建议把标注按长图坐标 `(0, 0)` 绘制。

## 11. 你需要提供的 UI 能力

Runtime 已经包含捕获、滚轮、窗口查找和拼接。你只需要在 UI 层提供这些薄接口：

```rust
fn current_selected_region(&self) -> Rect;
fn virtual_desktop_bounds(&self) -> LongShotRect;
fn set_passthrough_region(&mut self, region: LongShotRect);
fn show_longshot_preview(&mut self, image: LongShotImage);
fn ensure_timer_running(&mut self);
fn save_png_or_copy_clipboard(&mut self, pixels: Vec<u8>, width: i32, height: i32);
```

其中最重要的是 `set_passthrough_region`：长截屏时选区内部最好允许鼠标穿透，否则底层窗口可能收不到真实滚轮。不过即使浮层挡住，Runtime 也会用 `WindowFromPoint` + 排除 overlay 的方式去找下面窗口并投递滚轮。

## 12. 最小接入骨架

```rust
use std::time::Duration;
use crate::longshot::{
    LongShotImage, LongShotPoint, LongShotRect, LongShotRuntime, LongShotRuntimeEvent,
};

struct App {
    hwnd: HWND,
    longshot: LongShotRuntime,
}

impl App {
    fn on_longshot_button(&mut self) {
        let desktop = self.virtual_desktop_bounds();
        let selected = self.selected_region_as_longshot_rect();

        if let Ok(event) = self.longshot.start(desktop, self.hwnd.0 as isize, selected) {
            self.apply_longshot_event(event);
            self.set_passthrough_region(selected);
            self.ensure_timer_running();
        }
    }

    fn on_mouse_wheel(&mut self, wheel_delta: i32, screen_x: i32, screen_y: i32) {
        let point = LongShotPoint { x: screen_x, y: screen_y };
        if self.longshot.handle_manual_wheel(point, wheel_delta) {
            self.ensure_timer_running();
        }
    }

    fn on_timer(&mut self) {
        for event in self.longshot.tick() {
            self.apply_longshot_event(event);
        }
    }

    fn on_auto_scroll_button(&mut self) {
        self.longshot.toggle_auto_scroll();
        self.ensure_timer_running();
    }

    fn on_save(&mut self) {
        if let Some(image) = self.longshot.finish_for_export(Duration::from_millis(500)) {
            self.save_png(image.pixels, image.width, image.height);
        }
    }

    fn apply_longshot_event(&mut self, event: LongShotRuntimeEvent) {
        match event {
            LongShotRuntimeEvent::Started { image } |
            LongShotRuntimeEvent::PreviewUpdated { image, .. } => {
                self.show_longshot_preview(image);
            }
            _ => {}
        }
    }
}
```

## 13. 常见问题

1. 图片必须是 RGBA。`windows.rs` 已经把 GDI 的 BGRA 转成 RGBA。
2. `desktop_bounds` 必须是虚拟桌面坐标，不要只传主屏 `(0, 0, 1920, 1080)`。
3. 滚动后不要立刻截图。Runtime 默认延迟 50ms。
4. 自动滚动必须跑 timer。只开启状态但不调用 `tick()`，它不会继续工作。
5. 如果你隐藏 overlay 后再保存，先调用 `finish_for_export(...)`，否则最后一帧可能还在后台线程里。

