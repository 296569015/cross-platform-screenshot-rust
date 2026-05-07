#[cfg(windows)]
extern crate windows as windows_api;

pub mod common;
pub mod config;
pub mod core;
pub mod dc_control;
pub mod longshot;
pub mod platform;
pub mod png;
pub mod raster;
pub mod res;
pub mod select_border;
pub mod select_edit;
pub mod uibase;

#[cfg(windows)]
pub mod win32;
#[cfg(windows)]
pub mod windlg;
#[cfg(windows)]
pub mod windows;
