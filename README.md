# Cross Platform Screenshot Rust

Rust rewrite of the original `D:\code\cross-platform-screenshot` prototype.

Current package version: `2.8.8`, Rust Edition 2021.

## Implemented

- Windows global hotkeys: `Ctrl+Shift+A`, `Ctrl+Alt+X`, `F8`
- System tray menu: take screenshot / quit
- Full virtual-desktop capture through Win32 GDI
- Fullscreen topmost overlay with dim mask and drag region selection
- Annotation tools: rectangle, arrow, line, freehand brush
- Undo / redo: `Ctrl+Z`, `Ctrl+Shift+Z`
- Save PNG through a native save-file dialog
- Copy to clipboard as `CF_DIB` plus registered `PNG` clipboard data
- Short GIF recording from the selected region
- Pin selected screenshots into an independent topmost window
- Manual and automatic long-screenshot stitching
- Pure Rust core state machine, annotation model, command history, raster drawing, and PNG encoder

## Long Screenshot Extraction

The long-screenshot implementation is isolated under `src/longshot/`:

- `stitcher.rs`: pure RGBA overlap detection and stitching
- `session.rs`: screenshot-tool friendly session wrapper
- `runtime.rs`: one-call Windows runtime for capture, wheel forwarding, background stitching, auto-scroll, and export
- `windows.rs`: Win32 capture, covered-window fallback, wheel forwarding, and worker thread

Integration notes for moving this feature into another Rust/Windows screenshot tool are in
[`docs/long_screenshot_integration.md`](docs/long_screenshot_integration.md).

## Architecture

```text
main.rs
  -> windlg.rs
  -> uibase.rs
  -> select_edit.rs + select_border.rs
  -> dc_control.rs
  -> windows/gifdlg.rs + windows/pindlg.rs
```

`src/win32.rs` is kept as a compatibility facade that re-exports `windlg::run`.

## Build

```powershell
cargo build --release
```

The executable is:

```text
target\release\cross-platform-screenshot-rust.exe
```

## Test

```powershell
cargo test
```
