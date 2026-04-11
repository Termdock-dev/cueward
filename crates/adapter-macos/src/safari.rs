use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use serde::Serialize;

use cueward_core::{Cue, CueSource};

use crate::MacosError;
use crate::applescript::run_capture;

/// Core Data epoch: 2001-01-01 00:00:00 UTC
const CORE_DATA_EPOCH: i64 = 978_307_200;
const TAB_SEPARATOR: &str = "---TAB_SEP---";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SafariTab {
    pub window_id: i64,
    pub window_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub index: usize,
    pub title: String,
    pub url: String,
    pub active: bool,
}

fn history_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Safari/History.db")
}

fn to_core_data_timestamp(dt: DateTime<Utc>) -> f64 {
    (dt.timestamp() - CORE_DATA_EPOCH) as f64
}

fn from_core_data_timestamp(ts: f64) -> DateTime<Utc> {
    Utc.timestamp_opt(ts as i64 + CORE_DATA_EPOCH, 0)
        .single()
        .unwrap_or_default()
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
            Some('s') => decoded.push_str(TAB_SEPARATOR),
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

fn extract_profile(window_name: &str, tab_title: &str) -> Option<String> {
    let expected_suffix = format!(" — {tab_title}");
    window_name
        .strip_suffix(&expected_suffix)
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_tab_line(line: &str) -> Option<SafariTab> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 6 {
        return None;
    }

    let window_id = parts[0].trim().parse().ok()?;
    let window_name = decode_field(parts[1]);
    let index: usize = parts[2].trim().parse().ok()?;
    let title = decode_field(parts[3]);
    let url = decode_field(parts[4]);
    let active = parts[5].trim() == "true";
    Some(SafariTab {
        window_id,
        window_name,
        profile: None,
        index,
        title,
        url,
        active,
    })
}

fn parse_tabs_output(stdout: &str) -> Vec<SafariTab> {
    let mut tabs: Vec<SafariTab> = stdout
        .split(TAB_SEPARATOR)
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_tab_line)
        .collect();

    let mut profiles_by_window = HashMap::new();
    for tab in &tabs {
        if tab.active {
            if let Some(profile) = extract_profile(&tab.window_name, &tab.title) {
                profiles_by_window.insert(tab.window_id, profile);
            }
        }
    }

    for tab in &mut tabs {
        tab.profile = profiles_by_window.get(&tab.window_id).cloned();
    }

    tabs
}

fn safari_script_prelude() -> String {
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
    "#,
        separator = TAB_SEPARATOR,
    )
}

fn build_tabs_script() -> String {
    format!(
        r#"
        {prelude}
        tell application "Safari"
            set output to ""
            repeat with w in every window
                set winId to id of w
                set winName to my encode_field(name of w)
                set activeTabIndex to index of current tab of w
                repeat with t in tabs of w
                    set tabIndex to (index of t) - 1
                    set tabTitle to my encode_field(name of t)
                    set tabURL to my encode_field(URL of t)
                    if (index of t) is activeTabIndex then
                        set isActive to "true"
                    else
                        set isActive to "false"
                    end if
                    set output to output & winId & tab & winName & tab & tabIndex & tab & tabTitle & tab & tabURL & tab & isActive & "{separator}"
                end repeat
            end repeat
            return output
        end tell
    "#,
        prelude = safari_script_prelude(),
        separator = TAB_SEPARATOR,
    )
}

fn build_active_tab_script() -> String {
    format!(
        r#"
        {prelude}
        tell application "Safari"
            if (count of windows) is 0 then
                return ""
            end if
            set w to front window
            set t to current tab of w
            set winId to id of w
            set winName to my encode_field(name of w)
            set tabIndex to (index of t) - 1
            set tabTitle to my encode_field(name of t)
            set tabURL to my encode_field(URL of t)
            return winId & tab & winName & tab & tabIndex & tab & tabTitle & tab & tabURL & tab & "true"
        end tell
    "#,
        prelude = safari_script_prelude(),
    )
}

pub fn capture(since: DateTime<Utc>) -> Result<Vec<Cue>, MacosError> {
    let db_path = history_db_path();

    if !db_path.exists() {
        return Err(MacosError::PermissionDenied(
            db_path.to_string_lossy().into_owned(),
        ));
    }

    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        if e.to_string().contains("unable to open") {
            MacosError::PermissionDenied(db_path.to_string_lossy().into_owned())
        } else {
            MacosError::Sqlite(e)
        }
    })?;

    let since_ts = to_core_data_timestamp(since);

    let mut stmt = conn.prepare(
        "SELECT v.visit_time, v.title, i.url \
         FROM history_visits v \
         JOIN history_items i ON v.history_item = i.id \
         WHERE v.visit_time > ?1 \
         ORDER BY v.visit_time DESC",
    )?;

    let cues = stmt
        .query_map([since_ts], |row| {
            let visit_time: f64 = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let url: String = row.get(2)?;
            Ok((visit_time, title, url))
        })?
        .filter_map(|r| r.ok())
        .map(|(visit_time, title, url)| Cue {
            source: CueSource::Safari,
            timestamp: from_core_data_timestamp(visit_time),
            content: title.clone().unwrap_or_default(),
            url: Some(url),
            title,
            tags: Vec::new(),
            attachment_segments: Vec::new(),
            metadata: HashMap::new(),
        })
        .collect();

    Ok(cues)
}

pub fn tabs(profile_filter: Option<&str>) -> Result<Vec<SafariTab>, MacosError> {
    let stdout = run_capture(&build_tabs_script(), "safari_tabs")?;
    let mut tabs = parse_tabs_output(&stdout);
    if let Some(profile) = profile_filter {
        tabs.retain(|tab| tab.profile.as_deref() == Some(profile));
    }
    Ok(tabs)
}

pub fn active() -> Result<Option<SafariTab>, MacosError> {
    let stdout = run_capture(&build_active_tab_script(), "safari_active")?;
    Ok(parse_tab_line(stdout.trim()))
}

#[cfg(test)]
mod tests {
    use super::{build_tabs_script, extract_profile, parse_tab_line, parse_tabs_output, TAB_SEPARATOR};

    #[test]
    fn extract_profile_from_window_name() {
        let profile = extract_profile("Ryugu — Google Gemini", "Google Gemini");

        assert_eq!(profile.as_deref(), Some("Ryugu"));
    }

    #[test]
    fn parse_tab_line_decodes_fields() {
        let line = "61998\tRyugu — Google\\tGemini\t0\tGoogle\\tGemini\thttps://gemini.google.com/app\ttrue";

        let tab = parse_tab_line(line).expect("tab");

        assert_eq!(tab.window_id, 61998);
        assert_eq!(tab.window_name, "Ryugu — Google\tGemini");
        assert_eq!(tab.profile, None);
        assert_eq!(tab.index, 0);
        assert_eq!(tab.title, "Google\tGemini");
        assert_eq!(tab.url, "https://gemini.google.com/app");
        assert!(tab.active);
    }

    #[test]
    fn parse_tabs_output_keeps_multiple_tabs() {
        let raw = concat!(
            "1\tWork — Mail\t0\tMail\thttps://mail.google.com\ttrue---TAB_SEP---",
            "1\tWork — Docs\t1\tDocs\thttps://docs.google.com\tfalse---TAB_SEP---"
        );

        let tabs = parse_tabs_output(raw);

        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].title, "Mail");
        assert_eq!(tabs[1].title, "Docs");
        assert_eq!(tabs[0].profile.as_deref(), Some("Work"));
        assert_eq!(tabs[1].profile.as_deref(), Some("Work"));
    }

    #[test]
    fn safari_script_escapes_record_separator() {
        let script = build_tabs_script();

        assert!(script.contains(TAB_SEPARATOR));
        assert!(script.contains("\\s"));
    }
}
