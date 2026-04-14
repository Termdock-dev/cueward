use std::process;

use chrono::{DateTime, Duration, Local, TimeZone};
use clap::Subcommand;

use super::helpers::{local_day_bounds, parse_datetime, parse_datetime_arg};

#[derive(Subcommand)]
pub(crate) enum RemindersAction {
    /// List reminders, optionally filtered by list name
    List {
        /// Filter by reminders list name
        #[arg(long)]
        list: Option<String>,
        /// Keep reminders due on or before this datetime/date
        #[arg(long)]
        due_before: Option<String>,
        /// Keep reminders due tomorrow
        #[arg(long)]
        due_tomorrow: bool,
    },
    /// List reminders due today
    Today,
    /// Create a reminder
    Create {
        /// Reminder title
        #[arg(long)]
        title: String,
        /// Due date or datetime
        #[arg(long)]
        due: Option<String>,
        /// Reminders list name
        #[arg(long, default_value = "Cueward")]
        list: String,
        /// Reminder notes
        #[arg(long)]
        notes: Option<String>,
        /// Priority value
        #[arg(long)]
        priority: Option<u8>,
    },
    /// Mark a reminder complete
    Complete {
        /// Match by title
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        title: Option<String>,
        /// Match by reminder id
        #[arg(long, required_unless_present = "title", conflicts_with = "title")]
        id: Option<String>,
    },
    /// Delete a reminder
    Delete {
        /// Match by title
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        title: Option<String>,
        /// Match by reminder id
        #[arg(long, required_unless_present = "title", conflicts_with = "title")]
        id: Option<String>,
    },
    /// Update a reminder
    Update {
        /// Match by title
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        title: Option<String>,
        /// Match by reminder id
        #[arg(long, required_unless_present = "title", conflicts_with = "title")]
        id: Option<String>,
        /// New title
        #[arg(long)]
        new_title: Option<String>,
        /// Replace due date or datetime
        #[arg(long)]
        due: Option<String>,
        /// Replace notes
        #[arg(long)]
        notes: Option<String>,
        /// Move to a different list
        #[arg(long)]
        list: Option<String>,
        /// Replace priority value
        #[arg(long)]
        priority: Option<u8>,
    },
}

pub(crate) fn dispatch_plan(title: String, notes: String, list: String) {
    match cueward_adapter_macos::plan::create_reminder(&title, &notes, &list) {
        Ok(()) => eprintln!("reminder created in {list}"),
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    }
}

fn reminder_selector(
    title: Option<String>,
    id: Option<String>,
) -> cueward_adapter_macos::reminders::ReminderSelector {
    match (title, id) {
        (Some(title), None) => cueward_adapter_macos::reminders::ReminderSelector::Title(title),
        (None, Some(id)) => cueward_adapter_macos::reminders::ReminderSelector::Id(id),
        _ => unreachable!("clap ensures exactly one selector"),
    }
}

fn parse_reminder_due_datetime(value: &str) -> Option<DateTime<Local>> {
    if let Some(parsed) = parse_datetime(value) {
        return Some(parsed);
    }

    let naive = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .or_else(|| Local.from_local_datetime(&naive).latest())
}

fn normalize_due_before_cutoff(value: &str) -> Result<DateTime<Local>, String> {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return date
            .and_hms_opt(23, 59, 59)
            .and_then(|ndt| Local.from_local_datetime(&ndt).single())
            .or_else(|| {
                date.and_hms_opt(23, 59, 59)
                    .and_then(|ndt| Local.from_local_datetime(&ndt).earliest())
            })
            .or_else(|| {
                date.and_hms_opt(23, 59, 59)
                    .and_then(|ndt| Local.from_local_datetime(&ndt).latest())
            })
            .ok_or_else(|| format!("error: invalid due-before datetime '{value}'"));
    }

    parse_datetime_arg("due-before", value)
}

fn filter_reminders(
    reminders: Vec<cueward_adapter_macos::reminders::ReminderItem>,
    due_before_dt: Option<DateTime<Local>>,
    tomorrow_bounds: Option<(DateTime<Local>, DateTime<Local>)>,
) -> Vec<cueward_adapter_macos::reminders::ReminderItem> {
    reminders
        .into_iter()
        .filter(|reminder| {
            let due = reminder
                .due_date
                .as_deref()
                .and_then(parse_reminder_due_datetime);
            match due_before_dt {
                Some(cutoff) => due.map(|due| due <= cutoff).unwrap_or(false),
                None => true,
            }
        })
        .filter(|reminder| match tomorrow_bounds {
            Some((from, to)) => reminder
                .due_date
                .as_deref()
                .and_then(parse_reminder_due_datetime)
                .map(|due| due >= from && due <= to)
                .unwrap_or(false),
            None => true,
        })
        .collect()
}

pub(crate) fn dispatch(action: RemindersAction) {
    match action {
        RemindersAction::List {
            list,
            due_before,
            due_tomorrow,
        } => {
            match cueward_adapter_macos::reminders::list(list.as_deref()) {
                Ok(reminders) => {
                    let due_before_dt = match due_before.as_deref() {
                        Some(value) => match normalize_due_before_cutoff(value) {
                            Ok(dt) => Some(dt),
                            Err(err) => {
                                eprintln!("{err}");
                                process::exit(1);
                            }
                        },
                        None => None,
                    };
                    let tomorrow_bounds = if due_tomorrow {
                        let tomorrow = Local::now() + Duration::days(1);
                        match local_day_bounds(tomorrow) {
                            Ok(bounds) => Some(bounds),
                            Err(err) => {
                                eprintln!("{err}");
                                process::exit(1);
                            }
                        }
                    } else {
                        None
                    };
                    let reminders = filter_reminders(reminders, due_before_dt, tomorrow_bounds);
                    println!("{}", serde_json::to_string_pretty(&reminders).unwrap());
                    eprintln!("{} reminder(s)", reminders.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        RemindersAction::Today => match cueward_adapter_macos::reminders::today() {
            Ok(reminders) => {
                println!("{}", serde_json::to_string_pretty(&reminders).unwrap());
                eprintln!("{} reminder(s) due today", reminders.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        RemindersAction::Create {
            title,
            due,
            list,
            notes,
            priority,
        } => {
            let due_dt = match due.as_deref() {
                Some(value) => match parse_datetime_arg("due", value) {
                    Ok(dt) => Some(dt),
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                },
                None => None,
            };
            match cueward_adapter_macos::reminders::create_reminder(
                &title,
                notes.as_deref().unwrap_or(""),
                &list,
                due_dt,
                priority,
            ) {
                Ok(()) => eprintln!("reminder created: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        RemindersAction::Complete { title, id } => {
            match cueward_adapter_macos::reminders::complete_reminder(reminder_selector(title, id)) {
                Ok(()) => eprintln!("reminder completed"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        RemindersAction::Delete { title, id } => {
            match cueward_adapter_macos::reminders::delete_reminder(reminder_selector(title, id)) {
                Ok(()) => eprintln!("reminder deleted"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        RemindersAction::Update {
            title,
            id,
            new_title,
            due,
            notes,
            list,
            priority,
        } => {
            let due_dt = match due.as_deref() {
                Some(value) => match parse_datetime_arg("due", value) {
                    Ok(dt) => Some(dt),
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                },
                None => None,
            };
            match cueward_adapter_macos::reminders::update_reminder(
                reminder_selector(title, id),
                new_title.as_deref(),
                due_dt,
                notes.as_deref(),
                list.as_deref(),
                priority,
            ) {
                Ok(()) => eprintln!("reminder updated"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone, Timelike};
    use cueward_adapter_macos::reminders::ReminderItem;

    use super::{filter_reminders, normalize_due_before_cutoff, parse_reminder_due_datetime};

    #[test]
    fn parse_reminder_due_datetime_accepts_adapter_format_without_timezone() {
        let due = parse_reminder_due_datetime("2026-04-16T09:00:00").expect("adapter due");

        assert_eq!(due.hour(), 9);
        assert_eq!(due.minute(), 0);
        assert_eq!(due.second(), 0);
    }

    #[test]
    fn normalize_due_before_cutoff_expands_date_only_to_end_of_day() {
        let cutoff = normalize_due_before_cutoff("2026-04-16").expect("cutoff");
        let same_day_due = Local
            .with_ymd_and_hms(2026, 4, 16, 15, 30, 0)
            .single()
            .expect("same day due");

        assert!(same_day_due <= cutoff);
        assert_eq!(cutoff.hour(), 23);
        assert_eq!(cutoff.minute(), 59);
        assert_eq!(cutoff.second(), 59);
    }

    #[test]
    fn filter_reminders_keeps_same_day_due_before_matches() {
        let reminders = vec![ReminderItem {
            id: "1".into(),
            title: "same day".into(),
            notes: String::new(),
            due_date: Some("2026-04-16T09:00:00".into()),
            completed: false,
            list_name: "Cueward".into(),
        }];

        let filtered = filter_reminders(
            reminders,
            Some(normalize_due_before_cutoff("2026-04-16").expect("cutoff")),
            None,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "same day");
    }

    #[test]
    fn filter_reminders_matches_tomorrow_against_adapter_due_format() {
        let reminders = vec![ReminderItem {
            id: "1".into(),
            title: "tomorrow".into(),
            notes: String::new(),
            due_date: Some("2026-04-16T09:00:00".into()),
            completed: false,
            list_name: "Cueward".into(),
        }];
        let tomorrow = Local
            .with_ymd_and_hms(2026, 4, 15, 12, 0, 0)
            .single()
            .expect("tomorrow seed")
            + chrono::Duration::days(1);
        let bounds = crate::commands::helpers::local_day_bounds(tomorrow).expect("bounds");

        let filtered = filter_reminders(reminders, None, Some(bounds));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "tomorrow");
    }
}
