use crate::MacosError;
use crate::applescript::{escape, escape_body, run};

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

/// Update a uniquely titled note, refusing ambiguous matches across accounts.
pub fn update_note(title: &str, body: &str, folder: &str) -> Result<(), MacosError> {
    let id = super::selection::resolve_title(title, folder)?;
    update_selected(&id, body, Some((title, folder)))
}

/// Update one note by its native Notes ID, preserving its current title.
pub fn update_note_by_id(id: &str, body: &str) -> Result<(), MacosError> {
    update_selected(id, body, None)
}

fn update_selected(id: &str, body: &str, guard: Option<(&str, &str)>) -> Result<(), MacosError> {
    let action = format!(
        "set originalName to name of targetNote\nset body of targetNote to {}\nset name of targetNote to originalName",
        escape_body(body)
    );
    run(
        &super::selection::action_script(id, guard, &action),
        "failed to update note",
    )
}

/// Delete a uniquely titled note, refusing ambiguous matches across accounts.
pub fn delete_note(title: &str, folder: &str) -> Result<(), MacosError> {
    let id = super::selection::resolve_title(title, folder)?;
    run(
        &super::selection::action_script(&id, Some((title, folder)), "delete targetNote"),
        "failed to delete note",
    )
}

/// Delete one note by its native Notes ID.
pub fn delete_note_by_id(id: &str) -> Result<(), MacosError> {
    run(
        &super::selection::action_script(id, None, "delete targetNote"),
        "failed to delete note",
    )
}

/// Move a uniquely titled note; resolve the source before creating a destination.
pub fn move_note(title: &str, from_folder: &str, to_folder: &str) -> Result<(), MacosError> {
    let id = super::selection::resolve_title(title, from_folder)?;
    move_selected(&id, to_folder, Some((title, from_folder)))
}

/// Move one note by its native Notes ID.
pub fn move_note_by_id(id: &str, to_folder: &str) -> Result<(), MacosError> {
    move_selected(id, to_folder, None)
}

fn move_selected(id: &str, to: &str, guard: Option<(&str, &str)>) -> Result<(), MacosError> {
    let destination = escape(to);
    let action = format!(
        r#"
        try
            set destFolder to folder "{destination}"
        on error
            make new folder with properties {{name:"{destination}"}}
            set destFolder to folder "{destination}"
        end try
        move targetNote to destFolder"#
    );
    run(
        &super::selection::action_script(id, guard, &action),
        "failed to move note",
    )
}

#[cfg(test)]
mod tests {
    use super::{create_note, delete_note};

    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    #[ignore = "requires explicit authorization to create and delete a note in the real Notes app"]
    fn create_note_supports_multiline_body() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let title = format!("Cueward create note multiline test {suffix}");
        let folder = "Cueward";

        let result = create_note(&title, "line1\nline2", folder);
        let _ = delete_note(&title, folder);

        assert!(
            result.is_ok(),
            "expected multiline note creation to succeed: {result:?}"
        );
    }
}
