mod capture;
mod windows;

pub use capture::{
    capture, capture_window, ensure_screenshot_file_exists, validate_display, validate_user_output_path,
    ScreenshotResult,
};
pub use windows::{list_capturable_windows, CapturableWindow, WindowBounds};

#[cfg(test)]
mod tests;
