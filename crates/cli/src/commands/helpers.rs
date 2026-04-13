use chrono::{DateTime, Local, TimeZone};

use super::Source;
use super::safari_ai::GeminiMode;

pub(crate) fn parse_datetime(s: &str) -> Option<DateTime<Local>> {
    use chrono::NaiveDateTime;

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        if let Some(dt) = Local
            .from_local_datetime(&ndt)
            .single()
            .or_else(|| Local.from_local_datetime(&ndt).earliest())
            .or_else(|| Local.from_local_datetime(&ndt).latest())
        {
            return Some(dt);
        }
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        if let Some(dt) = Local
            .from_local_datetime(&ndt)
            .single()
            .or_else(|| Local.from_local_datetime(&ndt).earliest())
            .or_else(|| Local.from_local_datetime(&ndt).latest())
        {
            return Some(dt);
        }
    }
    None
}

pub(crate) fn parse_datetime_arg(
    label: &str,
    value: &str,
) -> Result<DateTime<Local>, String> {
    parse_datetime(value).ok_or_else(|| format!("error: invalid {label} datetime '{value}'"))
}

pub(crate) fn parse_required_datetime_arg(
    label: &str,
    value: Option<&str>,
) -> Result<DateTime<Local>, String> {
    match value {
        Some(value) => parse_datetime_arg(label, value),
        None => Err(format!("error: missing {label} datetime")),
    }
}

pub(crate) fn validate_optional_output_path(
    label: &str,
    value: Option<&str>,
) -> Result<(), String> {
    if let Some(path) = value {
        if std::path::Path::new(path)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(format!(
                "error: {label} path must not contain parent directory components"
            ));
        }
    }
    Ok(())
}

pub(crate) fn local_day_bounds(
    now: DateTime<Local>,
) -> Result<(DateTime<Local>, DateTime<Local>), String> {
    let from = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .ok_or_else(|| "error: could not determine start of today".to_string())?;
    let to = now
        .date_naive()
        .and_hms_opt(23, 59, 59)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .ok_or_else(|| "error: could not determine end of today".to_string())?;
    Ok((from, to))
}

pub(crate) fn parse_duration(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if let Some(hours) = s.strip_suffix('h') {
        hours.parse().ok().map(chrono::Duration::hours)
    } else if let Some(days) = s.strip_suffix('d') {
        days.parse().ok().map(chrono::Duration::days)
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.parse().ok().map(chrono::Duration::minutes)
    } else {
        None
    }
}

pub(crate) fn source_name(src: &Source) -> &'static str {
    match src {
        Source::Safari => "safari",
        Source::Notes => "notes",
        Source::Messages => "messages",
        Source::All => unreachable!(),
    }
}

pub(crate) fn to_adapter_gemini_mode(mode: GeminiMode) -> cueward_adapter_macos::safari::GeminiMode {
    match mode {
        GeminiMode::Image => cueward_adapter_macos::safari::GeminiMode::Image,
        GeminiMode::DeepResearch => cueward_adapter_macos::safari::GeminiMode::DeepResearch,
        GeminiMode::Video => cueward_adapter_macos::safari::GeminiMode::Video,
        GeminiMode::Music => cueward_adapter_macos::safari::GeminiMode::Music,
    }
}

pub(crate) fn print_external(source: &str, json: &str) {
    let safe = json.replace("</external>", "&lt;/external&gt;");
    println!("<external source=\"cueward/{source}\">");
    println!("{safe}");
    println!("</external>");
}

pub(crate) fn bookmarks_target_folder(profile: Option<&str>, folder: Option<&str>) -> Option<String> {
    let profile = profile.map(str::trim).filter(|value| !value.is_empty());
    let folder = folder.map(str::trim).filter(|value| !value.is_empty());

    match (profile, folder) {
        (Some(profile), Some(folder)) => Some(format!("{profile}/{folder}")),
        (Some(profile), None) => Some(profile.to_string()),
        (None, Some(folder)) => Some(folder.to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Timelike;
    use chrono::{Local, TimeZone};

    use super::{
        bookmarks_target_folder, local_day_bounds, parse_datetime, validate_optional_output_path,
    };

    #[test]
    fn validate_optional_output_path_rejects_parent_components() {
        let result = validate_optional_output_path("--output", Some("../secret.png"));

        assert_eq!(
            result,
            Err("error: --output path must not contain parent directory components".to_string())
        );
    }

    #[test]
    fn local_day_bounds_covers_full_day() {
        let now = Local
            .with_ymd_and_hms(2026, 4, 11, 10, 30, 0)
            .single()
            .expect("local dt");

        let (from, to) = local_day_bounds(now).expect("bounds");

        assert_eq!(from.hour(), 0);
        assert_eq!(from.minute(), 0);
        assert_eq!(from.second(), 0);
        assert_eq!(to.hour(), 23);
        assert_eq!(to.minute(), 59);
        assert_eq!(to.second(), 59);
    }

    #[test]
    fn parse_datetime_accepts_ambiguous_local_time() {
        let parsed = parse_datetime("2026-11-01 01:30");

        assert!(parsed.is_some());
    }

    #[test]
    fn bookmarks_target_folder_prepends_profile_to_folder() {
        let folder = bookmarks_target_folder(Some("Ryugu"), Some("Work/AI Tools"));

        assert_eq!(folder, Some("Ryugu/Work/AI Tools".to_string()));
    }

    #[test]
    fn bookmarks_target_folder_uses_profile_as_root_when_folder_missing() {
        let folder = bookmarks_target_folder(Some("Ryugu"), None);

        assert_eq!(folder, Some("Ryugu".to_string()));
    }
}
