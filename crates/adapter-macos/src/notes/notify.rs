use crate::MacosError;
use crate::applescript::{escape, run};

/// Send a macOS notification via osascript.
pub fn notify(title: &str, message: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_msg = escape(message);

    let script = format!(r#"display notification "{escaped_msg}" with title "{escaped_title}""#);

    run(&script, "notification failed")
}
