mod capture;
mod windows;

pub use capture::{
    ScreenshotResult, capture, capture_window, ensure_screenshot_file_exists, validate_display,
    validate_user_output_path,
};
pub(crate) use capture::{capture_window_frame, ensure_cache_dir};
pub use windows::{
    CapturableWindow, WindowBounds, WindowScope, list_capturable_windows, list_windows,
};

#[cfg(test)]
mod tests;
