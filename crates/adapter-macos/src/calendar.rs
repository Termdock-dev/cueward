use chrono::{DateTime, Local};
use serde::Serialize;

use crate::MacosError;
use crate::applescript::{applescript_date_block, escape, escape_body, run, run_capture};

#[path = "calendar_eventkit.rs"]
mod eventkit;

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
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 7 {
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
    if let Some(events) = eventkit::list_events(from, to, calendar_filter)? {
        return Ok(events);
    }

    let from_block = applescript_date_block("fromDate", &from);
    let to_block = applescript_date_block("toDate", &to);

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

        {from_block}
        {to_block}

        tell application "Calendar"
            set output to ""
            {cal_filter_block}
            repeat with aCal in targetCals
                set calName to my encode_field(name of aCal)
                set evts to (events of aCal whose (start date < toDate) and (end date > fromDate))
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

    let stdout = run_capture(&script, "list_events")?;
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
    let start_block = applescript_date_block("startDate", &start);
    let end_block = applescript_date_block("endDate", &end);

    let notes_prop = notes
        .map(|n| format!(r#", description:{}"#, escape_body(n)))
        .unwrap_or_default();
    let location_prop = location
        .map(|l| format!(r#", location:{}"#, escape_body(l)))
        .unwrap_or_default();

    let target_cal_block = match calendar_name {
        Some(name) => {
            let escaped = escape(name);
            format!(r#"set targetCal to calendar "{escaped}""#)
        }
        None => "set targetCal to first calendar".to_string(),
    };

    let script = format!(
        r#"
        {start_block}
        {end_block}
        tell application "Calendar"
            {target_cal_block}
            make new event at end of events of targetCal with properties {{summary:"{escaped_title}", start date:startDate, end date:endDate{notes_prop}{location_prop}}}
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
    let start_block = applescript_date_block("startDate", &start);

    let script = format!(
        r#"
        {start_block}
        tell application "Calendar"
            set targetCal to calendar "{escaped_cal}"
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

fn build_update_script(
    title: &str,
    calendar_name: Option<&str>,
    new_title: Option<&str>,
    new_start: Option<&DateTime<Local>>,
    new_end: Option<&DateTime<Local>>,
    notes: Option<&str>,
    location: Option<&str>,
) -> String {
    let escaped_title = escape(title);
    let target_cal_block = match calendar_name {
        Some(name) => {
            let escaped = escape(name);
            format!(
                r#"set targetCals to (calendars whose name is "{escaped}")
            if targetCals is {{}} then
                error "calendar not found: {escaped}"
            end if"#
            )
        }
        None => "set targetCals to calendars".to_string(),
    };
    let start_block = new_start
        .map(|dt| applescript_date_block("newStartDate", dt))
        .unwrap_or_default();
    let end_block = new_end
        .map(|dt| applescript_date_block("newEndDate", dt))
        .unwrap_or_default();

    let mut actions = Vec::new();
    if let Some(new_title) = new_title {
        actions.push(format!(
            r#"set summary of targetEvt to "{}""#,
            escape(new_title)
        ));
    }
    match (new_start.is_some(), new_end.is_some()) {
        (true, true) => {
            actions.push("set start date of targetEvt to newStartDate".to_string());
            actions.push("set end date of targetEvt to newEndDate".to_string());
        }
        (true, false) => {
            actions.push(
                "set originalDuration to (end date of targetEvt) - (start date of targetEvt)"
                    .to_string(),
            );
            actions.push("set start date of targetEvt to newStartDate".to_string());
            actions
                .push("set end date of targetEvt to newStartDate + originalDuration".to_string());
        }
        (false, true) => {
            actions.push("set end date of targetEvt to newEndDate".to_string());
        }
        (false, false) => {}
    }
    if let Some(notes) = notes {
        actions.push(format!(
            "set description of targetEvt to {}",
            escape_body(notes)
        ));
    }
    if let Some(location) = location {
        actions.push(format!(
            "set location of targetEvt to {}",
            escape_body(location)
        ));
    }

    format!(
        r#"
        {start_block}
        {end_block}
        tell application "Calendar"
            {target_cal_block}
            set matchingEvts to {{}}
            repeat with aCal in targetCals
                repeat with evt in (events of aCal whose summary is "{escaped_title}")
                    copy evt to end of matchingEvts
                end repeat
            end repeat
            if matchingEvts is {{}} then
                error "event not found: {escaped_title}"
            end if
            if (count of matchingEvts) > 1 then
                error "event title is ambiguous: {escaped_title}"
            end if
            set targetEvt to item 1 of matchingEvts
            {actions}
        end tell
        "#,
        actions = actions.join("\n            "),
    )
}

/// Update a calendar event matched by title and optional calendar.
pub fn update_event(
    title: &str,
    calendar_name: Option<&str>,
    new_title: Option<&str>,
    new_start: Option<DateTime<Local>>,
    new_end: Option<DateTime<Local>>,
    notes: Option<&str>,
    location: Option<&str>,
) -> Result<(), MacosError> {
    if new_title.is_none()
        && new_start.is_none()
        && new_end.is_none()
        && notes.is_none()
        && location.is_none()
    {
        return Err(MacosError::Other("no calendar updates specified".into()));
    }

    let script = build_update_script(
        title,
        calendar_name,
        new_title,
        new_start.as_ref(),
        new_end.as_ref(),
        notes,
        location,
    );

    run(&script, "failed to update calendar event")
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::{build_update_script, parse_event_line, parse_events_output};

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

    #[test]
    fn create_event_script_uses_escape_body_for_multiline_fields() {
        let escaped = crate::applescript::escape_body("Line 1\nLine 2");

        assert_eq!(escaped, "\"Line 1\" & linefeed & \"Line 2\"");
    }

    #[test]
    fn update_event_script_preserves_duration_when_only_start_changes() {
        let new_start = Local
            .with_ymd_and_hms(2026, 4, 16, 14, 0, 0)
            .single()
            .expect("new start");

        let script = build_update_script(
            "顧問會議",
            Some("Work"),
            Some("顧問會議（改期）"),
            Some(&new_start),
            None,
            None,
            None,
        );

        assert!(script.contains(r#"set targetCals to (calendars whose name is "Work")"#));
        assert!(
            script.contains(r#"repeat with evt in (events of aCal whose summary is "顧問會議")"#)
        );
        assert!(script.contains(
            "set originalDuration to (end date of targetEvt) - (start date of targetEvt)"
        ));
        assert!(script.contains("set start date of targetEvt to newStartDate"));
        assert!(script.contains("set end date of targetEvt to newStartDate + originalDuration"));
        assert!(script.contains(r#"set summary of targetEvt to "顧問會議（改期）""#));
    }
}
