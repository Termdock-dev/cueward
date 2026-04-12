use crate::MacosError;
use crate::applescript::{escape, escape_body, run};

/// Create a reminder in Apple Reminders.
pub fn create_reminder(title: &str, notes: &str, list: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_list = escape(list);
    let notes_expr = escape_body(notes);

    let script = format!(
        r#"
        tell application "Reminders"
            try
                set targetList to list "{escaped_list}"
            on error
                make new list with properties {{name:"{escaped_list}"}}
                set targetList to list "{escaped_list}"
            end try
            make new reminder at targetList with properties {{name:"{escaped_title}", body:{notes_expr}}}
        end tell
        "#
    );

    run(&script, "failed to create reminder")
}
