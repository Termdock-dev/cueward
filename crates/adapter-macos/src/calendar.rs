use std::process::Command;

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::applescript::{escape, run};
use crate::MacosError;

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

/// Parse a tab-separated event line from the AppleScript output.
/// Fields: title \t start \t end \t calendar \t location \t notes \t all_day
pub fn parse_event_line(line: &str) -> Option<CalendarEvent> {
    let parts: Vec<&str> = line.splitn(7, '\t').collect();
    if parts.len() < 7 {
        return None;
    }
    let title = parts[0].to_string();
    if title.is_empty() {
        return None;
    }
    Some(CalendarEvent {
        title,
        start: parts[1].to_string(),
        end: parts[2].to_string(),
        calendar: parts[3].to_string(),
        location: parts[4].to_string(),
        notes: parts[5].to_string(),
        all_day: parts[6].trim() == "true",
    })
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
        tell application "Calendar"
            set fromDate to date "{from_str}"
            set toDate to date "{to_str}"
            set output to ""
            {cal_filter_block}
            repeat with aCal in targetCals
                set calName to name of aCal
                set evts to (events of aCal whose start date >= fromDate and start date <= toDate)
                repeat with evt in evts
                    set evtTitle to summary of evt
                    set evtStart to (start date of evt) as «class isot» as string
                    set evtEnd to (end date of evt) as «class isot» as string
                    if location of evt is missing value then
                        set evtLoc to ""
                    else
                        set evtLoc to location of evt
                    end if
                    if description of evt is missing value then
                        set evtNotes to ""
                    else
                        set evtNotes to description of evt
                    end if
                    set evtAllDay to allday event of evt
                    set output to output & evtTitle & tab & evtStart & tab & evtEnd & tab & calName & tab & evtLoc & tab & evtNotes & tab & evtAllDay & linefeed
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
    let events = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(parse_event_line)
        .collect();

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
