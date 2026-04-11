use std::process::Command;

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::MacosError;
use crate::applescript::{escape, run};

const EVENT_SEPARATOR: &str = "---EVENT_SEP---";

#[derive(Serialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start: String,
    pub end: String,
    pub calendar: String,
    pub location: String,
    pub notes: String,
    pub all_day: bool,
}

/// Format a local datetime as "YYYY-MM-DD HH:MM:SS" for AppleScript date parsing.
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

/// Parse a tab-separated event line from the AppleScript output.
/// Fields: title \t start \t end \t calendar \t location \t notes \t all_day
pub fn parse_event_line(line: &str) -> Option<CalendarEvent> {
    let parts: Vec<&str> = line.splitn(7, '\t').collect();
    if parts.len() < 7 {
        return None;
    }
    let title = decode_field(parts[0]);
    if title.is_empty() {
        return None;
    }
    Some(CalendarEvent {
        title,
        start: parts[1].to_string(),
        end: parts[2].to_string(),
        calendar: decode_field(parts[3]),
        location: decode_field(parts[4]),
        notes: decode_field(parts[5]),
        all_day: parts[6].trim() == "true",
    })
}

fn parse_events_output(stdout: &str) -> Vec<CalendarEvent> {
    stdout
        .split(EVENT_SEPARATOR)
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_event_line)
        .collect()
}

/// List calendar events in the given time range, optionally filtered by calendar name.
pub fn list_events(
    from: DateTime<Local>,
    to: DateTime<Local>,
    calendar_filter: Option<&str>,
) -> Result<Vec<CalendarEvent>, MacosError> {
    let from_str = format_for_applescript(&from);
    let to_str = format_for_applescript(&to);

    let cal_filter_block = match calendar_filter {
        Some(name) => {
            let escaped = escape(name);
            format!(
                r#"set targetCals to (calendars whose name is "{escaped}")
            if targetCals is {{}} then
                return ""
            end if"#
            )
        }
        None => "set targetCals to calendars".to_string(),
    };

    let script = format!(
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

        tell application "Calendar"
            set fromDate to date "{from_str}"
            set toDate to date "{to_str}"
            set output to ""
            {cal_filter_block}
            repeat with aCal in targetCals
                set calName to my encode_field(name of aCal)
                set evts to (events of aCal whose start date >= fromDate and start date <= toDate)
                repeat with evt in evts
                    set evtTitle to my encode_field(summary of evt)
                    set evtStart to (start date of evt) as «class isot» as string
                    set evtEnd to (end date of evt) as «class isot» as string
                    if location of evt is missing value then
                        set evtLoc to ""
                    else
                        set evtLoc to my encode_field(location of evt)
                    end if
                    if description of evt is missing value then
                        set evtNotes to ""
                    else
                        set evtNotes to my encode_field(description of evt)
                    end if
                    set evtAllDay to allday event of evt
                    set output to output & evtTitle & tab & evtStart & tab & evtEnd & tab & calName & tab & evtLoc & tab & evtNotes & tab & evtAllDay & "{EVENT_SEPARATOR}"
                end repeat
            end repeat
            return output
        end tell
        "#
    );

    let raw = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| MacosError::Other(format!("osascript: {e}")))?;

    if !raw.status.success() {
        let stderr = String::from_utf8_lossy(&raw.stderr);
        return Err(MacosError::Other(format!("list_events: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&raw.stdout);
    let events = parse_events_output(&stdout);

    Ok(events)
}

/// Create a calendar event.
pub fn create_event(
    title: &str,
    start: DateTime<Local>,
    end: DateTime<Local>,
    calendar_name: Option<&str>,
    notes: Option<&str>,
    location: Option<&str>,
) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let start_str = format_for_applescript(&start);
    let end_str = format_for_applescript(&end);

    let notes_prop = notes
        .map(|n| format!(r#", description:"{}""#, escape(n)))
        .unwrap_or_default();
    let location_prop = location
        .map(|l| format!(r#", location:"{}""#, escape(l)))
        .unwrap_or_default();

    let target_cal_block = match calendar_name {
        Some(name) => {
            let escaped = escape(name);
            format!(r#"set targetCal to calendar "{escaped}""#)
        }
        None => "set targetCal to default calendar".to_string(),
    };

    let script = format!(
        r#"
        tell application "Calendar"
            {target_cal_block}
            make new event at end of events of targetCal with properties {{summary:"{escaped_title}", start date:date "{start_str}", end date:date "{end_str}"{notes_prop}{location_prop}}}
        end tell
        "#
    );

    run(&script, "failed to create calendar event")
}

/// Delete a calendar event matched by title and start date.
pub fn delete_event(
    title: &str,
    start: DateTime<Local>,
    calendar_name: &str,
) -> Result<(), MacosError> {
    let escaped_title = escape(title);
    let escaped_cal = escape(calendar_name);
    let start_str = format_for_applescript(&start);

    let script = format!(
        r#"
        tell application "Calendar"
            set targetCal to calendar "{escaped_cal}"
            set startDate to date "{start_str}"
            set matchingEvts to (events of targetCal whose summary is "{escaped_title}" and start date is startDate)
            if matchingEvts is {{}} then
                error "event not found: {escaped_title}"
            end if
            repeat with evt in matchingEvts
                delete evt
            end repeat
        end tell
        "#
    );

    run(&script, "failed to delete calendar event")
}

#[cfg(test)]
mod tests {
    use super::{parse_event_line, parse_events_output};

    #[test]
    fn parse_event_line_unescapes_sanitized_fields() {
        let line = "Team\\tSync\t2026-04-11T09:00:00Z\t2026-04-11T10:00:00Z\tWork\\nCal\tRoom\\rA\tLine 1\\nLine 2\ttrue";

        let event = parse_event_line(line).expect("event");

        assert_eq!(event.title, "Team\tSync");
        assert_eq!(event.calendar, "Work\nCal");
        assert_eq!(event.location, "Room\rA");
        assert_eq!(event.notes, "Line 1\nLine 2");
        assert!(event.all_day);
    }

    #[test]
    fn parse_events_output_keeps_multiline_notes() {
        let raw = concat!(
            "Title\t2026-04-11T09:00:00Z\t2026-04-11T10:00:00Z\tWork\tDesk\tLine 1\\nLine 2\tfalse",
            "---EVENT_SEP---"
        );

        let events = parse_events_output(raw);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].notes, "Line 1\nLine 2");
    }
}
