#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    std::process::exit(cross_platform_screenshot_rust::win32::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The interactive screenshot app is currently implemented for Windows.");
}
