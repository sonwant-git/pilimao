//! `pilimao-win32-shell`: 1x/2x/3x Nearest-Neighbor BGRA compositor and native borderless
//! 3-window Win32 shell (`SetWindowRgn`, `BeginDeferWindowPos`, `WM_DROPFILES`, classic shortcuts).

pub mod hit_test;
pub mod renderer;
pub mod shortcuts;
pub mod surface;
pub mod win32;

use thiserror::Error;

pub use hit_test::{EqHit, MainHit, PlHit, hit_test_eq, hit_test_main, hit_test_playlist};
pub use renderer::{ShellRenderer, UiRenderState, VisMode};
pub use shortcuts::{Modifiers, ShortcutAction, map_virtual_key};
pub use surface::BgraSurface;
pub use win32::{ShellLaunchConfig, run_win32_shell};

/// Errors produced by the Win32 shell.
#[derive(Debug, Error)]
pub enum ShellError {
    #[error("the native Win32 GUI shell is only available on Windows 10/11")]
    WindowsOnly,
    #[error("Win32 API error: {0}")]
    Win32(String),
    #[error("audio engine initialization failed: {0}")]
    AudioInit(String),
}
