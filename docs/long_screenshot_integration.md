# 长截屏能力剥离与集成指南

本文档给公司截图工具接入长截屏使用。目标是让模型或开发同学尽量少理解原项目 UI，直接复制 `src/longshot/`，再把几个调用点接到现有 Win32 截图流程里。

## 1. 先复制哪些文件

把本项目整个目录复制到公司项目：

```text
src/longshot/
  mod.rs
  types.rs
  stitcher.rs
  session.rs
  windows.rs
```

在公司项目 `lib.rs` 或 `main.rs` 的模块声明处加：

```rust
pub mod longshot;
```

如果公司项目只想复用拼接算法，并继续用自己的截屏和滚轮转发代码，可以不复制 `windows.rs`，但要同时删掉 `mod.rs` 里的这一行：

```rust
#[cfg(windows)]
pub mod windows;
```

## 2. 依赖要求

纯拼接与会话层只用 Rust 标准库，不依赖 `image`、`rayon`、`GDI+`。

如果使用 `src/longshot/windows.rs`，`Cargo.toml` 里 `windows` 依赖需要包含这些 feature：

```toml
windows = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_Storage_Xps",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
] }
```

本项目为了兼容原代码，在 `lib.rs` 里用了：

```rust
#[cfg(windows)]
extern crate windows as windows_api;
```

如果公司项目直接叫 `windows`，把 `src/longshot/windows.rs` 中所有 `crate::windows_api::...` 替换成 `windows::...` 即可。

## 3. 数据格式约定

长截屏模块只认一种图片格式：

```rust
LongShotImage {
    width: i32,
    height: i32,
    pixels: Vec<u8>, // RGBA, top-down, 长度 = width * height * 4
}
```

如果公司项目已有 `Image` 类型，比如：

```rust
struct Image {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}
```

写两个转换函数即可：

```rust
fn to_longshot_image(img: &Image) -> longshot::LongShotImage {
    longshot::LongShotImage {
        width: img.width,
        height: img.height,
        pixels: img.pixels.clone(),
    }
}

fn from_longshot_image(img: longshot::LongShotImage) -> Image {
    Image {
        width: img.width,
        height: img.height,
        pixels: img.pixels,
    }
}
```

坐标约定：

```rust
LongShotRect { x, y, w, h }
```

`x/y` 是相对“全虚拟桌面截图”的坐标。多屏时虚拟桌面左上角可能不是 `(0, 0)`，Win32 捕获时需要加上 `desktop_bounds.x/y`。

## 4. 模块职责

`types.rs`

基础类型：`LongShotRect`、`LongShotPoint`、`LongShotImage`。

`stitcher.rs`

核心拼接算法。输入连续帧 RGBA，自动寻找重叠区，输出拼接后的 RGBA。

`session.rs`

给截图工具用的推荐封装。负责：

- 根据选区高度生成默认拼接参数
- 保存长截屏会话状态
- 防止同一次滚动重复追加
- 裁掉 Win32 捕获偶发的底部黑色掉帧
- 输出最终长图

`windows.rs`

可选 Win32 适配层。负责：

- GDI `BitBlt` 区域捕获
- `PrintWindow(PW_RENDERFULLCONTENT)` 兜底捕获被遮挡窗口
- 向底层窗口投递 `WM_MOUSEWHEEL`
- 后台捕获线程 `LongShotCaptureWorker`

## 5. 在公司架构里的落点

按你们现有架构，推荐这样接：

```text
select_border.rs
  增加 LongScreenshot 工具栏按钮

windlg.rs
  保存 LongShotSession
  处理长截屏按钮点击、滚轮、timer、后台捕获结果

dc_control.rs
  如果不用 longshot::windows.rs，就把现有 GDI/GDI+ 截图函数封装成 LongShotImage

uibase.rs
  不需要理解长截屏算法，只负责按钮和预览 UI
```

## 6. windlg.rs 需要新增的状态

在主窗口状态结构里加这些字段：

```rust
use crate::longshot::{
    LongShotAppendStatus, LongShotImage, LongShotPoint, LongShotRect,
    LongShotSession, LongShotSessionOptions,
};

#[cfg(windows)]
use crate::longshot::windows::{
    LongShotCaptureRequest, LongShotCaptureResponse, LongShotCaptureWorker,
};

struct WindlgState {
    longshot: LongShotSession,
    longshot_active: bool,
    longshot_source_region: LongShotRect,
    longshot_generation: u64,
    longshot_capture_worker: Option<LongShotCaptureWorker>,
    longshot_capture_in_flight: bool,
    longshot_pending_seq: u64,
    longshot_last_scroll_target: isize,
    longshot_auto_scroll: bool,
    longshot_auto_stalls: i32,
}
```

初始化：

```rust
let state = WindlgState {
    longshot: LongShotSession::default(),
    longshot_active: false,
    longshot_source_region: LongShotRect::default(),
    longshot_generation: 0,
    longshot_capture_worker: LongShotCaptureWorker::spawn(),
    longshot_capture_in_flight: false,
    longshot_pending_seq: 0,
    longshot_last_scroll_target: 0,
    longshot_auto_scroll: false,
    longshot_auto_stalls: 0,
};
```

## 7. 点击长截屏按钮时启动会话

在 `select_border.rs` 的工具栏命令里加：

```rust
LongScreenshot,
```

在 `windlg.rs` 的工具栏点击处理里：

```rust
ToolbarCommand::LongScreenshot => {
    self.start_longshot();
}
```

实现 `start_longshot`：

```rust
fn start_longshot(&mut self) -> bool {
    let selected = self.current_selected_region(); // 公司项目已有选区
    if selected.w <= 0 || selected.h <= 0 {
        return false;
    }

    let Some(full_capture) = self.current_full_desktop_image() else {
        return false;
    };

    let source = LongShotRect::new(selected.x, selected.y, selected.w, selected.h);
    let full = LongShotImage {
        width: full_capture.width,
        height: full_capture.height,
        pixels: full_capture.pixels.clone(),
    };

    let options = LongShotSessionOptions::tuned_for_region_height(source.h);
    if self.longshot.start_from_full_capture(source, &full, options).is_err() {
        return false;
    }

    self.longshot_active = true;
    self.longshot_source_region = source;
    self.longshot_generation = self.longshot_generation.saturating_add(1);
    self.longshot_capture_in_flight = false;
    self.longshot_pending_seq = 0;
    self.longshot_last_scroll_target = 0;
    self.longshot_auto_scroll = false;
    self.longshot_auto_stalls = 0;

    let preview = self.longshot.output_image().unwrap();
    self.show_longshot_preview(preview);

    // 关键：让选区内部可以把滚轮给到底层窗口。
    // 如果你们的分层窗口支持穿透，就把 source 设置成穿透区域。
    self.set_passthrough_region(source);

    true
}
```

## 8. 处理用户手动滚轮

长截屏只处理向下滚动。收到鼠标滚轮后：

```rust
fn on_mouse_wheel(&mut self, wheel_delta: i32, screen_x: i32, screen_y: i32) {
    if !self.longshot_active || wheel_delta >= 0 {
        return;
    }

    let seq = self.longshot.next_scroll_seq();
    self.longshot_pending_seq = seq;

    let screen_point = LongShotPoint {
        x: screen_x,
        y: screen_y,
    };

    let ok = crate::longshot::windows::post_mouse_wheel_at(
        screen_point,
        wheel_delta,
        self.hwnd.0 as isize,
        &mut self.longshot_last_scroll_target,
    );
    if !ok {
        return;
    }

    // 建议 35-80ms 后截下一帧，太早页面还没滚完。
    self.set_timer_after_ms(50, TimerKind::LongShotCapture);
}
```

如果公司项目已经用低级鼠标钩子把滚轮直接放给底层窗口，那么无需 `post_mouse_wheel_at`，只需要在观察到选区内有向下滚轮时调用：

```rust
let seq = self.longshot.next_scroll_seq();
self.longshot_pending_seq = seq;
self.set_timer_after_ms(28, TimerKind::LongShotCapture);
```

## 9. 到时间后截取下一帧

推荐用后台线程，避免 UI 卡顿：

```rust
fn request_longshot_capture(&mut self) {
    if self.longshot_capture_in_flight {
        return;
    }

    let desktop_bounds = self.virtual_desktop_bounds_as_longshot_rect();
    let region = self.longshot_source_region;

    if !self.longshot.can_accept_frame_height(region.h) {
        self.longshot_auto_scroll = false;
        return;
    }

    let request = LongShotCaptureRequest {
        generation: self.longshot_generation,
        seq: self.longshot_pending_seq,
        desktop_bounds,
        overlay_hwnd: self.hwnd.0 as isize,
        region,
    };

    if let Some(worker) = &self.longshot_capture_worker {
        if worker.request(request) {
            self.longshot_capture_in_flight = true;
            return;
        }
    }

    // 没有 worker 时走同步兜底。
    let frame = crate::longshot::windows::capture_region_or_covered_window(
        request.desktop_bounds,
        request.overlay_hwnd,
        request.region,
    );
    self.handle_longshot_frame(request.generation, request.seq, frame);
}
```

轮询 worker：

```rust
fn poll_longshot_worker(&mut self) {
    let responses = self
        .longshot_capture_worker
        .as_ref()
        .map(LongShotCaptureWorker::drain_responses)
        .unwrap_or_default();

    for response in responses {
        self.handle_longshot_frame(response.generation, response.seq, response.frame);
    }
}
```

处理帧：

```rust
fn handle_longshot_frame(
    &mut self,
    generation: u64,
    seq: u64,
    frame: Option<LongShotImage>,
) {
    if generation != self.longshot_generation {
        return;
    }
    self.longshot_capture_in_flight = false;

    let Some(frame) = frame else {
        self.longshot_auto_stalls += 1;
        return;
    };

    let outcome = self.longshot.append_frame_for_scroll(seq, frame);
    match outcome.status {
        LongShotAppendStatus::Appended => {
            self.longshot_auto_stalls = 0;
            let preview = self.longshot.output_image().unwrap();
            self.show_longshot_preview(preview);
        }
        LongShotAppendStatus::Duplicate | LongShotAppendStatus::Rejected => {
            self.longshot_auto_stalls += 1;
        }
        LongShotAppendStatus::TooTall => {
            self.longshot_auto_scroll = false;
        }
        _ => {}
    }

    if self.longshot_auto_stalls >= 5 {
        self.longshot_auto_scroll = false;
    }
}
```

## 10. 自动滚动

自动滚动按钮建议只在长截屏模式下显示。

点击自动滚动按钮：

```rust
fn toggle_longshot_auto_scroll(&mut self) {
    if !self.longshot_active {
        return;
    }
    self.longshot_auto_scroll = !self.longshot_auto_scroll;
    self.longshot_auto_stalls = 0;
    if self.longshot_auto_scroll {
        self.set_timer_after_ms(1, TimerKind::LongShotAutoScroll);
    }
}
```

timer 中执行：

```rust
fn run_longshot_auto_scroll(&mut self) {
    if !self.longshot_auto_scroll || !self.longshot_active {
        return;
    }

    let source = self.longshot_source_region;
    let screen_point = LongShotPoint {
        x: self.desktop_origin_x + source.x + source.w / 2,
        y: self.desktop_origin_y + source.y + source.h / 2,
    };

    let wheel_delta = -40; // 约等于 -0.33 个滚轮格
    let ok = crate::longshot::windows::post_mouse_wheel_at(
        screen_point,
        wheel_delta,
        self.hwnd.0 as isize,
        &mut self.longshot_last_scroll_target,
    );

    if ok {
        let seq = self.longshot.next_scroll_seq();
        self.longshot_pending_seq = seq;
        self.set_timer_after_ms(45, TimerKind::LongShotCapture);
        self.set_timer_after_ms(120, TimerKind::LongShotAutoScroll);
    } else {
        self.longshot_auto_stalls += 1;
        if self.longshot_auto_stalls >= 5 {
            self.longshot_auto_scroll = false;
        }
    }
}
```

## 11. 保存、复制、贴图

保存前先等正在进行的捕获结束，再取最终图片：

```rust
fn longshot_output_for_export(&mut self) -> Option<LongShotImage> {
    self.longshot_auto_scroll = false;
    self.poll_longshot_worker();

    // 如果你们有 worker.wait_for_response，可以等 300-500ms。
    // 不等也可以，只是最后一次滚动可能来不及拼进去。

    self.longshot.output_image()
}
```

导出长图时，标注坐标建议相对长图 `(0, 0)` 绘制：

```rust
let mut image = self.longshot_output_for_export()?;
for annotation in self.annotations.iter() {
    draw_annotation(&mut image, annotation, 0, 0);
}
save_png_or_copy_clipboard(image.pixels, image.width, image.height);
```

普通截图仍然按原逻辑：先按选区 crop，再按选区 origin 绘制标注。

## 12. 如果不用 windows.rs，如何接 dc_control.rs

公司项目已有 `dc_control.rs` 和 GDI+。只要提供这两个函数即可：

```rust
fn capture_longshot_frame(region: LongShotRect) -> Option<LongShotImage> {
    let image = dc_control::capture_region(region.x, region.y, region.w, region.h)?;
    Some(LongShotImage {
        width: image.width,
        height: image.height,
        pixels: image.pixels,
    })
}

fn scroll_under_cursor(screen_x: i32, screen_y: i32, wheel_delta: i32) -> bool {
    // 可以复用公司项目已有鼠标消息转发。
    // 要点：目标窗口必须是截图浮层下面的真实窗口。
    true
}
```

然后继续使用 `LongShotSession` 做拼接。

## 13. 关键参数说明

默认调参入口：

```rust
LongShotSessionOptions::tuned_for_region_height(selected.h)
```

内部参数含义：

```rust
min_overlap_rows       // 两帧至少要匹配多少行
max_overlap_rows       // 最大搜索重叠行数
min_append_rows        // 每次至少新增多少行，小于它按重复帧处理
duplicate_score        // 整帧重复阈值
reliable_match_score   // 可靠匹配阈值，越小越严格
acceptable_match_score // 可接受匹配阈值
ambiguous_score_gap    // 第一名和第二名差距太小时认为有歧义
max_output_height      // 默认 16000，防止无限滚动吃内存
```

建议先不改参数。只有遇到特定网页拼接失败，再调：

- 拼不上：略微增大 `acceptable_match_score`
- 重复区域被拼进去：增大 `min_append_rows`
- 重复纹理页面错位：增大 `ambiguous_score_gap`，或把 `append_on_unreliable_match` 设为 `false`

## 14. 常见坑

1. RGBA 顺序错了会导致匹配分数异常。GDI DIB 通常是 BGRA，必须转成 RGBA。
2. `LongShotRect` 是相对虚拟桌面的选区，调用 Win32 `BitBlt` 时要加 `desktop_bounds.x/y`。
3. 截图浮层如果挡住选区，底层窗口收不到滚轮。要么设置选区穿透，要么用 `post_mouse_wheel_at` 找到浮层下面的窗口并投递滚轮。
4. 滚动后不要立刻截图，建议延迟 35-80ms。
5. 自动滚动必须设置失败上限，连续 5 次没有追加就停。
6. 导出前最好 flush 一次正在飞行的捕获请求，否则最后一次滚动可能没进最终长图。

## 15. 最小主流程

```rust
// 1. 启动
let first = full_capture.crop(selected).unwrap();
let options = LongShotSessionOptions::tuned_for_region_height(selected.h);
longshot.start(selected, first, options).unwrap();

// 2. 每次向下滚轮
let seq = longshot.next_scroll_seq();
post_or_forward_wheel(...);
delay_50ms_then_capture_region(selected);

// 3. 捕获返回
let outcome = longshot.append_frame_for_scroll(seq, frame);
if outcome.appended() {
    let preview = longshot.output_image().unwrap();
    show_preview(preview);
}

// 4. 保存
let final_image = longshot.output_image().unwrap();
save_png(final_image.pixels, final_image.width, final_image.height);
```

