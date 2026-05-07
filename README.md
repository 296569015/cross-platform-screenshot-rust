# Cross Platform Screenshot Rust

Rust rewrite of the original `D:\code\cross-platform-screenshot` prototype.

## Implemented

- Windows global hotkeys: `Ctrl+Shift+A`, `Ctrl+Alt+X`, `F8`
- System tray menu: take screenshot / quit
- Full virtual-desktop capture through Win32 GDI
- Fullscreen topmost overlay with dim mask and drag region selection
- Annotation tools: rectangle, arrow, line, freehand brush
- Undo / redo: `Ctrl+Z`, `Ctrl+Shift+Z`
- Save PNG through a native save-file dialog
- Copy to clipboard as `CF_DIB` plus registered `PNG` clipboard data
- Manual and automatic long-screenshot stitching
- Pure Rust core state machine, annotation model, command history, raster drawing, and PNG encoder

The Rust implementation keeps the user-facing behavior of the C++ app, but uses a software Win32/GDI pipeline instead of the original DXGI Desktop Duplication + ANGLE renderer. That keeps the rewrite compact and dependency-light while preserving the screenshot workflow.

## Build

```powershell
cargo build --release --offline
```

The executable is:

```text
target\release\cross-platform-screenshot-rust.exe
```

## Test

```powershell
cargo test --offline
```
