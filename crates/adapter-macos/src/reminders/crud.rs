use chrono::{DateTime, Local};

use crate::MacosError;
use crate::applescript::{applescript_date_block, escape, escape_body, run};

use super::list;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReminderSelector {
    Id(String),
    Title(String),
}

fn build_list_lookup_block(list_name: &str) -> String {
    let escaped = escape(list_name);
    format!(
        r#"try
                set targetList to list "{escaped}"
            on error
                make new list with properties {{name:"{escaped}"}}
                set targetList to list "{escaped}"
            end try"#
    )
}

fn build_reminder_lookup_script(reminder_id: &str, action_block: &str) -> String {
    let escaped_id = escape(reminder_id);
    format!(
        r#"
        tell application "Reminders"
            set targetReminder to missing value
            repeat with aList in lists
                repeat with aReminder in reminders of aList
                    if id of aReminder is "{escaped_id}" then
                        set targetReminder to aReminder
                        exit repeat
                    end if
                end repeat
                if targetReminder is not missing value then
                    exit repeat
                end if
            end repeat
            if targetReminder is missing value then
                error "reminder not found: {escaped_id}"
            end if
            {action_block}
        end tell
        "#
    )
}

fn resolve_selector(selector: ReminderSelector) -> Result<String, MacosError> {
    match selector {
        ReminderSelector::Id(id) => Ok(id),
        ReminderSelector::Title(title) => {
            let matches = list(None)?
                .into_iter()
                .filter(|reminder| reminder.title == title)
                .collect::<Vec<_>>();
            match matches.len() {
                0 => Err(MacosError::Other(format!("reminder not found: {title}"))),
                1 => Ok(matches[0].id.clone()),
                count => Err(MacosError::Other(format!(
                    "reminder title is ambiguous: {title} ({count} matches)"
                ))),
            }
        }
    }
}

/// Create a reminder in Apple Reminders.
pub fn create_reminder(
    title: &str,
    notes: &str,
    list: &str,
    due: Option<DateTime<Local>>,
    priority: Option<u8>,
) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let notes_expr = escape_body(notes);
    let target_list_block = build_list_lookup_block(list);
    let due_block = due
        .as_ref()
        .map(|due| applescript_date_block("dueDate", due))
        .unwrap_or_default();
    let due_prop = if due.is_some() {
        ", due date:dueDate".to_string()
    } else {
        String::new()
    };
    let priority_prop = priority
        .map(|priority| format!(", priority:{priority}"))
        .unwrap_or_default();

    let script = format!(
        r#"
        {due_block}
        tell application "Reminders"
            {target_list_block}
            make new reminder at targetList with properties {{name:"{escaped_title}", body:{notes_expr}{due_prop}{priority_prop}}}
        end tell
        "#
    );

    run(&script, "failed to create reminder")
}

/// Mark a reminder complete by id or unique title.
pub fn complete_reminder(selector: ReminderSelector) -> Result<(), MacosError> {
    let reminder_id = resolve_selector(selector)?;
    let script = build_reminder_lookup_script(&reminder_id, "set completed of targetReminder to true");

    run(&script, "failed to complete reminder")
}

/// Delete a reminder by id or unique title.
pub fn delete_reminder(selector: ReminderSelector) -> Result<(), MacosError> {
    let reminder_id = resolve_selector(selector)?;
    let script = build_reminder_lookup_script(&reminder_id, "delete targetReminder");

    run(&script, "failed to delete reminder")
}

/// Update a reminder by id or unique title.
pub fn update_reminder(
    selector: ReminderSelector,
    new_title: Option<&str>,
    due: Option<DateTime<Local>>,
    notes: Option<&str>,
    list: Option<&str>,
    priority: Option<u8>,
) -> Result<(), MacosError> {
    if new_title.is_none() && due.is_none() && notes.is_none() && list.is_none() && priority.is_none() {
        return Err(MacosError::Other("no reminder updates specified".to_string()));
    }

    let reminder_id = resolve_selector(selector)?;
    let due_block = due
        .as_ref()
        .map(|due| applescript_date_block("dueDate", due))
        .unwrap_or_default();

    let mut actions = Vec::new();
    if let Some(new_title) = new_title {
        actions.push(format!(r#"set name of targetReminder to "{}""#, escape(new_title)));
    }
    if due.is_some() {
        actions.push("set due date of targetReminder to dueDate".to_string());
    }
    if let Some(notes) = notes {
        actions.push(format!(
            "set body of targetReminder to {}",
            escape_body(notes)
        ));
    }
    if let Some(priority) = priority {
        actions.push(format!("set priority of targetReminder to {priority}"));
    }
    if let Some(list) = list {
        actions.push(build_list_lookup_block(list));
        actions.push("move targetReminder to targetList".to_string());
    }

    let script = format!(
        r#"
        {due_block}
        {lookup}
        "#,
        lookup = build_reminder_lookup_script(&reminder_id, &actions.join("\n            ")),
    );

    run(&script, "failed to update reminder")
}

#[cfg(test)]
mod tests {
    use super::build_reminder_lookup_script;

    #[test]
    fn reminder_lookup_script_matches_id() {
        let script = build_reminder_lookup_script("reminder-id", "delete targetReminder");

        assert!(script.contains(r#"if id of aReminder is "reminder-id" then"#));
        assert!(script.contains("delete targetReminder"));
    }
}
