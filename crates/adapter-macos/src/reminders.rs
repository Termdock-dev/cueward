use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
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

fn local_datetime_components(dt: &DateTime<Local>) -> (i32, u32, u32, u32) {
    (
        dt.year(),
        dt.month(),
        dt.day(),
        dt.num_seconds_from_midnight(),
    )
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
            Some('s') => decoded.push_str(REMINDER_SEPARATOR),
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

fn reminders_script_prelude() -> String {
    format!(
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
            set escaped_text to my replace_text("{separator}", "\\s", escaped_text)
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
    "#,
        separator = REMINDER_SEPARATOR,
    )
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
                    if completed of aReminder then
                        set reminderCompleted to "true"
                    else
                        set reminderCompleted to "false"
                    end if
                    set output to output & reminderTitle & tab & reminderNotes & tab & reminderDue & tab & reminderCompleted & tab & listName & "{REMINDER_SEPARATOR}"
                end repeat
            end repeat
            return output
        end tell
        "#,
        prelude = reminders_script_prelude(),
    )
}

pub fn list(list_filter: Option<&str>) -> Result<Vec<ReminderItem>, MacosError> {
    let stdout = run_capture(&build_list_script(list_filter), "list_reminders")?;
    Ok(parse_reminders_output(&stdout))
}

fn build_today_script(from: &str, to: &str) -> String {
    let from_dt = DateTime::parse_from_rfc3339(from)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
        .unwrap_or_else(Local::now);
    let to_dt = DateTime::parse_from_rfc3339(to)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
        .unwrap_or_else(Local::now);
    let (from_year, from_month, from_day, from_seconds) = local_datetime_components(&from_dt);
    let (to_year, to_month, to_day, to_seconds) = local_datetime_components(&to_dt);

    format!(
        r#"
        {prelude}
        tell application "Reminders"
            set fromDate to current date
            set year of fromDate to {from_year}
            set month of fromDate to {from_month}
            set day of fromDate to {from_day}
            set time of fromDate to {from_seconds}
            set toDate to current date
            set year of toDate to {to_year}
            set month of toDate to {to_month}
            set day of toDate to {to_day}
            set time of toDate to {to_seconds}
            set output to ""
            repeat with aList in lists
                set listName to my encode_field(name of aList)
                set matchingReminders to (reminders of aList whose due date is not missing value and due date is greater than or equal to fromDate and due date is less than or equal to toDate)
                repeat with aReminder in matchingReminders
                    set reminderTitle to my encode_field(name of aReminder)
                    if body of aReminder is missing value then
                        set reminderNotes to ""
                    else
                        set reminderNotes to my encode_field(body of aReminder)
                    end if
                    set reminderDue to my format_reminder_date(due date of aReminder)
                    if completed of aReminder then
                        set reminderCompleted to "true"
                    else
                        set reminderCompleted to "false"
                    end if
                    set output to output & reminderTitle & tab & reminderNotes & tab & reminderDue & tab & reminderCompleted & tab & listName & "{REMINDER_SEPARATOR}"
                end repeat
            end repeat
            return output
        end tell
        "#,
        prelude = reminders_script_prelude(),
        from_year = from_year,
        from_month = from_month,
        from_day = from_day,
        from_seconds = from_seconds,
        to_year = to_year,
        to_month = to_month,
        to_day = to_day,
        to_seconds = to_seconds,
    )
}

pub fn today() -> Result<Vec<ReminderItem>, MacosError> {
    let now = Local::now();
    let from = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .ok_or_else(|| MacosError::Other("failed to determine start of today".into()))?;
    let to = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .ok_or_else(|| MacosError::Other("failed to determine end of today".into()))?;

    let stdout = run_capture(
        &build_today_script(&from.to_rfc3339(), &to.to_rfc3339()),
        "today_reminders",
    )?;
    Ok(parse_reminders_output(&stdout))
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::{
        REMINDER_SEPARATOR, build_list_script, build_today_script, parse_reminder_line,
        parse_reminders_output,
    };

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
        let from = Local
            .with_ymd_and_hms(2026, 4, 11, 0, 0, 0)
            .single()
            .expect("from");
        let to = Local
            .with_ymd_and_hms(2026, 4, 11, 23, 59, 59)
            .single()
            .expect("to");
        let today_script = build_today_script(&from.to_rfc3339(), &to.to_rfc3339());

        assert!(!list_script.contains("class isot"));
        assert!(!today_script.contains("class isot"));
        assert!(list_script.contains("format_reminder_date"));
        assert!(today_script.contains("format_reminder_date"));
        assert!(list_script.contains("set reminderCompleted to \"true\""));
        assert!(today_script.contains("set matchingReminders to"));
    }

    #[test]
    fn parse_reminder_line_unescapes_separator_escape() {
        let line = "One\tContains\\sMarker\t\ttrue\tInbox";

        let reminder = parse_reminder_line(line).expect("reminder");

        assert_eq!(
            reminder.notes,
            format!("Contains{REMINDER_SEPARATOR}Marker")
        );
    }
}
