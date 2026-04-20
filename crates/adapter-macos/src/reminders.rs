use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use serde::Serialize;

use crate::MacosError;
use crate::applescript::{escape, run_capture};

mod crud;
mod eventkit;

pub use crud::{
    ReminderSelector, complete_reminder, create_reminder, delete_reminder, update_reminder,
};

const REMINDER_SEPARATOR: &str = "---REMINDER_SEP---";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ReminderItem {
    pub id: String,
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
    let (id, parts) = match parts.len() {
        6 => (decode_field(parts[0]), &parts[1..]),
        5 => (String::new(), &parts[..]),
        _ => return None,
    };

    let title = decode_field(parts[0]);
    if title.is_empty() {
        return None;
    }

    let due_date = match parts[2].trim() {
        "" => None,
        value => Some(value.to_string()),
    };

    Some(ReminderItem {
        id,
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

fn build_target_lists_block(list_filter: Option<&str>) -> String {
    match list_filter {
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
    }
}

fn reminder_rows_block(filter_clause: Option<&str>) -> String {
    let filter_clause = filter_clause.unwrap_or("set shouldInclude to true");
    format!(
        r#"
                set reminderProps to properties of reminders of aList
                repeat with reminderInfo in reminderProps
                    set reminderDueValue to due date of reminderInfo
                    {filter_clause}
                    if shouldInclude then
                        set reminderId to my encode_field(id of reminderInfo)
                        set reminderTitle to my encode_field(name of reminderInfo)
                        if body of reminderInfo is missing value then
                            set reminderNotes to ""
                        else
                            set reminderNotes to my encode_field(body of reminderInfo)
                        end if
                        if reminderDueValue is missing value then
                            set reminderDue to ""
                        else
                            set reminderDue to my format_reminder_date(reminderDueValue)
                        end if
                        if completed of reminderInfo then
                            set reminderCompleted to "true"
                        else
                            set reminderCompleted to "false"
                        end if
                        set output to output & reminderId & tab & reminderTitle & tab & reminderNotes & tab & reminderDue & tab & reminderCompleted & tab & listName & "{REMINDER_SEPARATOR}"
                    end if
                end repeat
        "#,
    )
}

fn build_list_script(list_filter: Option<&str>) -> String {
    let list_filter_block = build_target_lists_block(list_filter);
    let reminder_rows = reminder_rows_block(None);

    format!(
        r#"
        {prelude}
        tell application "Reminders"
            set output to ""
            {list_filter_block}
            repeat with aList in targetLists
                set listName to my encode_field(name of aList)
                {reminder_rows}
            end repeat
            return output
        end tell
        "#,
        prelude = reminders_script_prelude(),
        reminder_rows = reminder_rows,
    )
}

pub fn list(list_filter: Option<&str>) -> Result<Vec<ReminderItem>, MacosError> {
    if let Some(reminders) = eventkit::list(list_filter)? {
        return Ok(reminders);
    }

    let stdout = run_capture(&build_list_script(list_filter), "list_reminders")?;
    Ok(parse_reminders_output(&stdout))
}

fn build_due_range_script(from: &str, to: &str, list_filter: Option<&str>) -> String {
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
    let list_filter_block = build_target_lists_block(list_filter);
    let reminder_rows = r#"
                set reminderRefs to reminders of aList
                set reminderDueValues to due date of every reminder of aList
                set reminderCount to count of reminderDueValues
                repeat with idx from 1 to reminderCount
                    set reminderDueValue to item idx of reminderDueValues
                    if reminderDueValue is not missing value then
                        if reminderDueValue is greater than or equal to fromDate and reminderDueValue is less than or equal to toDate then
                            set reminderRef to item idx of reminderRefs
                            set reminderId to my encode_field(id of reminderRef)
                            set reminderTitle to my encode_field(name of reminderRef)
                            if body of reminderRef is missing value then
                                set reminderNotes to ""
                            else
                                set reminderNotes to my encode_field(body of reminderRef)
                            end if
                            set reminderDue to my format_reminder_date(reminderDueValue)
                            if completed of reminderRef then
                                set reminderCompleted to "true"
                            else
                                set reminderCompleted to "false"
                            end if
                            set output to output & reminderId & tab & reminderTitle & tab & reminderNotes & tab & reminderDue & tab & reminderCompleted & tab & listName & "{REMINDER_SEPARATOR}"
                        end if
                    end if
                end repeat
    "#;

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
            {list_filter_block}
            repeat with aList in targetLists
                set listName to my encode_field(name of aList)
                {reminder_rows}
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
        list_filter_block = list_filter_block,
        reminder_rows = reminder_rows,
    )
}

pub fn list_due_between(
    from: DateTime<Local>,
    to: DateTime<Local>,
    list_filter: Option<&str>,
) -> Result<Vec<ReminderItem>, MacosError> {
    if let Some(reminders) = eventkit::list(list_filter)? {
        return Ok(
            reminders
                .into_iter()
                .filter(|reminder| reminder_due_in_range(reminder, from, to))
                .collect(),
        );
    }

    let stdout = run_capture(
        &build_due_range_script(&from.to_rfc3339(), &to.to_rfc3339(), list_filter),
        "list_reminders_due_between",
    )?;
    Ok(parse_reminders_output(&stdout))
}

#[cfg(test)]
fn build_today_script(from: &str, to: &str) -> String {
    build_due_range_script(from, to, None)
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

    list_due_between(from, to, None).map_err(|err| match err {
        MacosError::Other(message) => MacosError::Other(format!("today_reminders: {message}")),
        other => other,
    })
}

fn reminder_due_in_range(reminder: &ReminderItem, from: DateTime<Local>, to: DateTime<Local>) -> bool {
    reminder
        .due_date
        .as_deref()
        .and_then(parse_due_date)
        .map(|due| due >= from && due <= to)
        .unwrap_or(false)
}

fn parse_due_date(value: &str) -> Option<DateTime<Local>> {
    let naive = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .or_else(|| Local.from_local_datetime(&naive).latest())
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
        let line = "reminder-id\tBuy\\tmilk\tLine 1\\nLine 2\t2026-04-11T08:00:00Z\tfalse\tWork";

        let reminder = parse_reminder_line(line).expect("reminder");

        assert_eq!(reminder.id, "reminder-id");
        assert_eq!(reminder.title, "Buy\tmilk");
        assert_eq!(reminder.notes, "Line 1\nLine 2");
        assert_eq!(reminder.due_date.as_deref(), Some("2026-04-11T08:00:00Z"));
        assert!(!reminder.completed);
        assert_eq!(reminder.list_name, "Work");
    }

    #[test]
    fn parse_reminders_output_keeps_multiple_items() {
        let raw = concat!(
            "id-1\tOne\t\t\tfalse\tInbox---REMINDER_SEP---",
            "id-2\tTwo\tNotes\t2026-04-11T08:00:00Z\ttrue\tWork---REMINDER_SEP---"
        );

        let reminders = parse_reminders_output(raw);

        assert_eq!(reminders.len(), 2);
        assert_eq!(reminders[0].id, "id-1");
        assert_eq!(reminders[0].title, "One");
        assert_eq!(reminders[0].due_date, None);
        assert_eq!(reminders[1].id, "id-2");
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
        assert!(list_script.contains("set reminderId to my encode_field(id of reminderInfo)"));
        assert!(list_script.contains("set reminderCompleted to \"true\""));
        assert!(today_script.contains("set reminderRefs to reminders of aList"));
        assert!(today_script.contains("set reminderDueValues to due date of every reminder of aList"));
    }

    #[test]
    fn list_script_uses_bulk_properties_lookup() {
        let script = build_list_script(None);

        assert!(script.contains("set reminderProps to properties of reminders of aList"));
        assert!(!script.contains("repeat with aReminder in reminders of aList"));
        assert!(!script.contains("id of aReminder"));
        assert!(!script.contains("name of aReminder"));
    }

    #[test]
    fn today_script_filters_due_dates_without_whose_clause() {
        let from = Local
            .with_ymd_and_hms(2026, 4, 11, 0, 0, 0)
            .single()
            .expect("from");
        let to = Local
            .with_ymd_and_hms(2026, 4, 11, 23, 59, 59)
            .single()
            .expect("to");
        let script = build_today_script(&from.to_rfc3339(), &to.to_rfc3339());

        assert!(script.contains("set reminderRefs to reminders of aList"));
        assert!(script.contains("set reminderDueValues to due date of every reminder of aList"));
        assert!(script.contains("if reminderDueValue is not missing value then"));
        assert!(script.contains("if reminderDueValue is greater than or equal to fromDate and reminderDueValue is less than or equal to toDate then"));
        assert!(script.contains("set reminderRef to item idx of reminderRefs"));
        assert!(!script.contains("set matchingReminders to"));
        assert!(!script.contains("set reminderProps to properties of reminders of aList"));
        assert!(!script.contains("whose due date is not missing value"));
    }

    #[test]
    fn parse_reminder_line_unescapes_separator_escape() {
        let line = "id-1\tOne\tContains\\sMarker\t\ttrue\tInbox";

        let reminder = parse_reminder_line(line).expect("reminder");

        assert_eq!(
            reminder.notes,
            format!("Contains{REMINDER_SEPARATOR}Marker")
        );
    }
}
