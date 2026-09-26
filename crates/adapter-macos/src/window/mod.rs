use serde::{Deserialize, Serialize};

use crate::screenshot::{CapturableWindow, ScreenshotResult};

mod actions;
mod bridge;
mod input;
mod input_lock;
mod input_target;
mod inspection;
pub(crate) mod process;
mod snapshot;
mod spaces;
mod target;
mod wait;

pub(crate) use bridge::AX_SUPPORT;

pub use crate::screenshot::{WindowScope, list_windows};
pub use actions::{ActionStatus, WindowActionResult, press, set_value};
pub use input::{
    BackgroundInputResult, BackgroundInputStatus, InputDelivery, InputRouteStatus, click, drag,
    input_status, key, scroll, type_text,
};
pub use inspection::{inspect_window, inspect_window_subtree, inspect_window_surface};
pub use snapshot::{SnapshotImage, WindowSnapshot, snapshot_window};
pub use spaces::{
    SpaceCatalog, SpaceDisplay, SpaceInfo, SpaceMoveResult, WindowSpaces, list_spaces,
    move_window_to_space, window_spaces,
};
pub use wait::{WaitCondition, WaitOptions, WaitResult, WaitSelector, WaitStatus, wait_for_window};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ax_tests;

#[cfg(test)]
mod snapshot_live_tests;

#[cfg(test)]
mod input_live_tests;

#[cfg(test)]
mod pointer_live_tests;

#[cfg(test)]
mod input_lifetime_tests;

#[cfg(test)]
mod menu_live_tests;

#[cfg(test)]
mod wait_live_tests;

/// Element frame in global macOS screen points, with a top-left origin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessibilityBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessibilityNode {
    pub r#ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_ref: Option<String>,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub actions: Vec<String>,
    pub settable_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<AccessibilityBounds>,
    pub child_count: usize,
    #[serde(skip_serializing)]
    pub(crate) fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessibilitySnapshot {
    pub window_id: u32,
    pub owner_pid: i32,
    pub surface: AccessibilitySurface,
    pub root_ref: String,
    pub nodes: Vec<AccessibilityNode>,
    pub truncated: bool,
}

/// Accessibility tree to explore using a verified window as context.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilitySurface {
    #[default]
    Window,
    Menu,
}

impl AccessibilitySurface {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Menu => "menu",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WindowInspectResult {
    pub window: CapturableWindow,
    pub accessibility: AccessibilitySnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<ScreenshotResult>,
}
