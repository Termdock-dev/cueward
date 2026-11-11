use crate::MacosError;
use crate::applescript::escape;

pub(super) fn unique_id(raw: &str) -> Result<String, MacosError> {
    let ids: Vec<String> = serde_json::from_str(raw)
        .map_err(|e| MacosError::Other(format!("invalid Notes identity response: {e}")))?;
    match ids.as_slice() {
        [id] if !id.is_empty() => Ok(id.clone()),
        [] => Err(MacosError::Other("note not found".into())),
        _ => Err(MacosError::Other(format!(
            "note title is ambiguous ({} matches); use --id",
            ids.len()
        ))),
    }
}

pub(super) fn resolve_title(title: &str, folder: &str) -> Result<String, MacosError> {
    let title = serde_json::to_string(title).map_err(|e| MacosError::Other(e.to_string()))?;
    let folder = serde_json::to_string(folder).map_err(|e| MacosError::Other(e.to_string()))?;
    // Query all accounts, so identically named folders cannot silently select one account.
    let script = format!(
        r#"JSON.stringify(Application('Notes').notes().filter(n => n.name() === {title} && n.container().name() === {folder}).map(n => n.id()))"#
    );
    unique_id(&super::scripting::run_jxa(&script)?)
}

pub(super) fn action_script(id: &str, guard: Option<(&str, &str)>, action: &str) -> String {
    let guard = guard
        .map(|(title, folder)| {
            format!(
                r#"
        if name of targetNote is not "{}" or name of container of targetNote is not "{}" then
            error "note changed after selection; inspect before retrying"
        end if"#,
                escape(title),
                escape(folder)
            )
        })
        .unwrap_or_default();
    format!(
        r#"
    tell application "Notes"
        set matchingNotes to (every note whose id is "{}")
        if (count of matchingNotes) is not 1 then
            error "note ID not found or ambiguous"
        end if
        set targetNote to item 1 of matchingNotes
        {guard}
        {action}
    end tell"#,
        escape(id)
    )
}

#[cfg(test)]
mod tests {
    use super::unique_id;

    #[test]
    fn identity_resolution_requires_exactly_one_nonempty_id() {
        assert_eq!(
            unique_id(r#"["x-coredata://note"]"#).unwrap(),
            "x-coredata://note"
        );
        assert!(
            unique_id("[]")
                .unwrap_err()
                .to_string()
                .contains("not found")
        );
        assert!(
            unique_id(r#"["a","b"]"#)
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
        assert!(unique_id("not json").is_err());
        assert!(unique_id(r#"[""]"#).is_err());
    }
}
