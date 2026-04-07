use crate::applescript::{escape, escape_body, run};
use crate::MacosError;

/// Create a note in Apple Notes with the given title and body.
pub fn create_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);
    let body_expr = escape_body(body);

    let script = format!(
        r#"
        tell application "Notes"
            try
                set targetFolder to folder "{escaped_folder}"
            on error
                make new folder with properties {{name:"{escaped_folder}"}}
                set targetFolder to folder "{escaped_folder}"
            end try
            make new note at targetFolder with properties {{name:"{escaped_title}", body:{body_expr}}}
        end tell
        "#
    );

    run(&script, "failed to create note")
}

/// Update an existing note's body by title.
pub fn update_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);
    let body_expr = escape_body(body);

    let script = format!(
        r#"
        tell application "Notes"
            set theNote to (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
            set body of theNote to {body_expr}
        end tell
        "#
    );

    run(&script, "failed to update note")
}

/// Delete a note by title from a specific folder.
pub fn delete_note(title: &str, folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_folder = escape(folder);

    let script = format!(
        r#"
        tell application "Notes"
            delete (first note of folder "{escaped_folder}" whose name is "{escaped_title}")
        end tell
        "#
    );

    run(&script, "failed to delete note")
}

/// Move a note to a different folder.
pub fn move_note(title: &str, from_folder: &str, to_folder: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_from = escape(from_folder);
    let escaped_to = escape(to_folder);

    let script = format!(
        r#"
        tell application "Notes"
            try
                set destFolder to folder "{escaped_to}"
            on error
                make new folder with properties {{name:"{escaped_to}"}}
                set destFolder to folder "{escaped_to}"
            end try
            set theNote to (first note of folder "{escaped_from}" whose name is "{escaped_title}")
            move theNote to destFolder
        end tell
        "#
    );

    run(&script, "failed to move note")
}

/// Send a macOS notification via osascript.
pub fn notify(title: &str, message: &str) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_msg = escape(message);

    let script = format!(
        r#"display notification "{escaped_msg}" with title "{escaped_title}""#
    );

    run(&script, "notification failed")
}
