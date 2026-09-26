use super::*;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenFileStatus {
    SentUnverified,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpenFileResult {
    pub status: OpenFileStatus,
    pub file: String,
    pub app: RunningApp,
    pub frontmost_pid_before: i32,
    pub frontmost_pid_after: i32,
    pub foreground_changed: bool,
}

fn open_request(
    bundle_id: Option<&str>,
    app_path: Option<&Path>,
    file: &Path,
    new_instance: bool,
) -> Result<Value, MacosError> {
    let mut request = launch_request(bundle_id, app_path)?;
    if !file.is_absolute() || !file.is_file() {
        return Err(MacosError::Other(
            "provide an absolute regular file path".into(),
        ));
    }
    let file = file
        .canonicalize()
        .map_err(|e| MacosError::Other(e.to_string()))?;
    let file = file
        .to_str()
        .ok_or_else(|| MacosError::Other("file path must be UTF-8".into()))?;
    request["action"] = json!("open");
    request["file"] = json!(file);
    request["new_instance"] = json!(new_instance);
    request["lock_dir"] = json!(crate::screenshot::ensure_cache_dir()?);
    Ok(request)
}

/// Ask an explicitly selected app to open one local file without requesting activation.
pub fn open_file(
    bundle_id: Option<&str>,
    app_path: Option<&Path>,
    file: &Path,
    new_instance: bool,
) -> Result<OpenFileResult, MacosError> {
    run(open_request(bundle_id, app_path, file, new_instance)?)
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
