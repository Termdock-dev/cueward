use crate::MacosError;
use std::process::Command;

pub(super) fn run_jxa(script: &str) -> Result<String, MacosError> {
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;
    if !output.status.success() {
        return Err(script_failure(&String::from_utf8_lossy(&output.stderr)));
    }
    String::from_utf8(output.stdout)
        .map_err(|e| MacosError::Other(format!("invalid Notes output: {e}")))
}

fn script_failure(stderr: &str) -> MacosError {
    let text = stderr.to_lowercase();
    if ["not allowed", "not authorized", "denied", "-1743"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        MacosError::PermissionDenied("Apple Notes access denied. Allow automation in System Settings > Privacy & Security > Automation".into())
    } else {
        MacosError::Other(format!("Notes script failed: {stderr}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notes_script_errors_preserve_automation_permission_guidance() {
        assert!(matches!(
            script_failure("Not authorized to send Apple events (-1743)"),
            MacosError::PermissionDenied(_)
        ));
        assert!(
            script_failure("permission denied")
                .to_string()
                .contains("Automation")
        );
        assert!(matches!(
            script_failure("note not found"),
            MacosError::Other(_)
        ));
    }
}
