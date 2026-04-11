use chrono::{DateTime, Local, TimeZone};
use serde::Serialize;

use crate::MacosError;
use crate::applescript::{escape, run_capture};

const REMINDER_SEPARATOR: &str = "---REMINDER_SEP---";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ReminderItem {
    pub title: String,
    pub notes: String,
    pub due_date: Option<String>,
    pub completed: bool,
    pub list_name: String,
}

fn format_for_applescript(dt: &DateTime<Local>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn decode_field(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('\\') => decoded.push('\\'),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }
    decoded
}

fn parse_reminder_line(line: &str) -> Option<ReminderItem> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 5 {
        return None;
    }

    let title = decode_field(parts[0]);
    if title.is_empty() {
        return None;
    }

    let due_date = match parts[2].trim() {
        "" => None,
        value => Some(value.to_string()),
    };

    Some(ReminderItem {
        title,
        notes: decode_field(parts[1]),
        due_date,
        completed: parts[3].trim() == "true",
        list_name: decode_field(parts[4]),
    })
}

fn parse_reminders_output(stdout: &str) -> Vec<ReminderItem> {
    stdout
        .split(REMINDER_SEPARATOR)
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_reminder_line)
        .collect()
}

fn reminders_script_prelude() -> &'static str {
    r#"
        on replace_text(find_text, replace_text, source_text)
            set previous_delimiters to AppleScript's text item delimiters
            set AppleScript's text item delimiters to find_text
            set chunks to every text item of source_text
            set AppleScript's text item delimiters to replace_text
            set replaced_text to chunks as text
            set AppleScript's text item delimiters to previous_delimiters
            return replaced_text
        end replace_text

        on encode_field(source_text)
            if source_text is missing value then
                return ""
            end if
            set escaped_text to my replace_text("\\", "\\\\", source_text)
            set escaped_text to my replace_text(tab, "\\t", escaped_text)
            set escaped_text to my replace_text(return, "\\r", escaped_text)
            set escaped_text to my replace_text(linefeed, "\\n", escaped_text)
            return escaped_text
        end encode_field

        on pad2(value_num)
            set value_text to value_num as string
            if value_num < 10 then
                return "0" & value_text
            end if
            return value_text
        end pad2

        on format_reminder_date(reminder_date)
            set y to year of reminder_date as integer
            set m to month of reminder_date as integer
            set d to day of reminder_date as integer
            set hh to hours of reminder_date as integer
            set mm to minutes of reminder_date as integer
            set ss to seconds of reminder_date as integer
            return (y as string) & "-" & my pad2(m) & "-" & my pad2(d) & "T" & my pad2(hh) & ":" & my pad2(mm) & ":" & my pad2(ss)
        end format_reminder_date
    "#
}

fn build_list_script(list_filter: Option<&str>) -> String {
    let list_filter_block = match list_filter {
        Some(name) => {
            let escaped = escape(name);
            format!(
                r#"set targetLists to (lists whose name is "{escaped}")
            if targetLists is {{}} then
                return ""
            end if"#
            )
        }
        None => "set targetLists to lists".to_string(),
    };

    format!(
        r#"
        {prelude}
        tell application "Reminders"
            set output to ""
            {list_filter_block}
            repeat with aList in targetLists
                set listName to my encode_field(name of aList)
                repeat with aReminder in reminders of aList
                    set reminderTitle to my encode_field(name of aReminder)
                    if body of aReminder is missing value then
                        set reminderNotes to ""
                    else
                        set reminderNotes to my encode_field(body of aReminder)
                    end if
                    if due date of aReminder is missing value then
                        set reminderDue to ""
                    else
                        set reminderDue to my format_reminder_date(due date of aReminder)
                    end if
                    set reminderCompleted to completed of aReminder
                    set output to output & reminderTitle & tab & reminderNotes & tab & reminderDue & tab & reminderCompleted & tab & listName & "{REMINDER_SEPARATOR}"
                end repeat
            end repeat
            return output
        end tell
        "#
        ,
        prelude = reminders_script_prelude(),
    )
}

pub fn list(list_filter: Option<&str>) -> Result<Vec<ReminderItem>, MacosError> {
    let stdout = run_capture(&build_list_script(list_filter), "list_reminders")?;
    Ok(parse_reminders_output(&stdout))
}

fn build_today_script(from: &str, to: &str) -> String {
    format!(
        r#"
        {prelude}
        tell application "Reminders"
            set fromDate to date "{from}"
            set toDate to date "{to}"
            set output to ""
            repeat with aList in lists
                set listName to my encode_field(name of aList)
                repeat with aReminder in reminders of aList
                    if due date of aReminder is not missing value then
                        set reminderDueDate to due date of aReminder
                        if reminderDueDate is greater than or equal to fromDate and reminderDueDate is less than or equal to toDate then
                            set reminderTitle to my encode_field(name of aReminder)
                            if body of aReminder is missing value then
                                set reminderNotes to ""
                            else
                                set reminderNotes to my encode_field(body of aReminder)
                            end if
                            set reminderDue to my format_reminder_date(reminderDueDate)
                            set reminderCompleted to completed of aReminder
                            set output to output & reminderTitle & tab & reminderNotes & tab & reminderDue & tab & reminderCompleted & tab & listName & "{REMINDER_SEPARATOR}"
                        end if
                    end if
                end repeat
            end repeat
            return output
        end tell
        "#,
        prelude = reminders_script_prelude(),
    )
}

pub fn today() -> Result<Vec<ReminderItem>, MacosError> {
    let now = Local::now();
    let from = format_for_applescript(
        &now.date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|dt| Local.from_local_datetime(&dt).single())
            .ok_or_else(|| MacosError::Other("failed to determine start of today".into()))?,
    );
    let to = format_for_applescript(
        &now.date_naive()
            .and_hms_opt(23, 59, 59)
            .and_then(|dt| Local.from_local_datetime(&dt).single())
            .ok_or_else(|| MacosError::Other("failed to determine end of today".into()))?,
    );

    let stdout = run_capture(&build_today_script(&from, &to), "today_reminders")?;
    Ok(parse_reminders_output(&stdout))
}

#[cfg(test)]
mod tests {
    use super::{build_list_script, build_today_script, parse_reminder_line, parse_reminders_output};

    #[test]
    fn parse_reminder_line_unescapes_fields() {
        let line = "Buy\\tmilk\tLine 1\\nLine 2\t2026-04-11T08:00:00Z\tfalse\tWork";

        let reminder = parse_reminder_line(line).expect("reminder");

        assert_eq!(reminder.title, "Buy\tmilk");
        assert_eq!(reminder.notes, "Line 1\nLine 2");
        assert_eq!(reminder.due_date.as_deref(), Some("2026-04-11T08:00:00Z"));
        assert!(!reminder.completed);
        assert_eq!(reminder.list_name, "Work");
    }

    #[test]
    fn parse_reminders_output_keeps_multiple_items() {
        let raw = concat!(
            "One\t\t\tfalse\tInbox---REMINDER_SEP---",
            "Two\tNotes\t2026-04-11T08:00:00Z\ttrue\tWork---REMINDER_SEP---"
        );

        let reminders = parse_reminders_output(raw);

        assert_eq!(reminders.len(), 2);
        assert_eq!(reminders[0].title, "One");
        assert_eq!(reminders[0].due_date, None);
        assert_eq!(reminders[1].title, "Two");
        assert_eq!(reminders[1].list_name, "Work");
    }

    #[test]
    fn reminder_scripts_do_not_use_invalid_isot_coercion() {
        let list_script = build_list_script(None);
        let today_script = build_today_script("2026-04-11 00:00:00", "2026-04-11 23:59:59");

        assert!(!list_script.contains("class isot"));
        assert!(!today_script.contains("class isot"));
        assert!(list_script.contains("format_reminder_date"));
        assert!(today_script.contains("format_reminder_date"));
    }
}
