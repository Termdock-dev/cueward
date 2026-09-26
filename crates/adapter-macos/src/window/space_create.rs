use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceCreateResult {
    pub status: super::super::ActionStatus,
    pub space_id: Option<u64>,
    pub display_id: Option<String>,
    pub reason: Option<String>,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
    pub visible_spaces_before: Vec<u64>,
    pub visible_spaces_after: Option<Vec<u64>>,
    pub visible_spaces_changed: Option<bool>,
}

/// Request a new native desktop without switching Spaces, then verify its identity and visibility.
pub fn create_space() -> Result<SpaceCreateResult, MacosError> {
    let directory = crate::screenshot::ensure_cache_dir()?;
    let lock = std::path::Path::new(&directory).join("space-create.lock");
    run(json!({
        "action": "create", "caller_pid": std::process::id(), "lock_path": lock,
    }))
}

#[cfg(test)]
#[path = "space_create_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "space_create_live_tests.rs"]
mod live_tests;

#[cfg(test)]
#[path = "space_create_request_tests.rs"]
mod request_tests;
