use std::process;

use clap::Subcommand;
use chrono::Local;

use super::helpers::{local_day_bounds, parse_datetime_arg, parse_required_datetime_arg};

#[derive(Subcommand)]
pub(crate) enum CalendarAction {
    /// List events in a time range (default: next 24h)
    List {
        /// Start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        from: Option<String>,
        /// End datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        to: Option<String>,
        /// Filter by calendar name
        #[arg(long)]
        calendar: Option<String>,
    },
    /// List today's events (00:00 to 23:59)
    Today {
        /// Filter by calendar name
        #[arg(long)]
        calendar: Option<String>,
    },
    /// Create a calendar event
    Create {
        /// Event title
        #[arg(long)]
        title: String,
        /// Start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        start: String,
        /// End datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        end: String,
        /// Calendar name (uses default calendar if omitted)
        #[arg(long)]
        calendar: Option<String>,
        /// Notes/description
        #[arg(long)]
        notes: Option<String>,
        /// Location
        #[arg(long)]
        location: Option<String>,
    },
    /// Delete a calendar event by title and start datetime
    Delete {
        /// Event title
        #[arg(long)]
        title: String,
        /// Start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        start: String,
        /// Calendar name
        #[arg(long)]
        calendar: String,
    },
    /// Update a calendar event matched by title
    Update {
        /// Event title
        #[arg(long)]
        title: String,
        /// Calendar name (searches all calendars if omitted)
        #[arg(long)]
        calendar: Option<String>,
        /// New title
        #[arg(long)]
        new_title: Option<String>,
        /// New start datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        new_start: Option<String>,
        /// New end datetime (RFC3339 or "YYYY-MM-DD HH:MM")
        #[arg(long)]
        new_end: Option<String>,
        /// Replace notes/description
        #[arg(long)]
        notes: Option<String>,
        /// Replace location
        #[arg(long)]
        location: Option<String>,
    },
}

pub(crate) fn dispatch(action: CalendarAction) {
    match action {
        CalendarAction::Today { calendar } => {
            let now = Local::now();
            let (from, to) = match local_day_bounds(now) {
                Ok(bounds) => bounds,
                Err(err) => {
                    eprintln!("{err}");
                    process::exit(1);
                }
            };
            match cueward_adapter_macos::calendar::list_events(from, to, calendar.as_deref()) {
                Ok(events) => {
                    println!("{}", serde_json::to_string_pretty(&events).unwrap());
                    eprintln!("{} event(s)", events.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        CalendarAction::List { from, to, calendar } => {
            let now = Local::now();
            let from_dt = match from.as_deref() {
                Some(value) => match parse_required_datetime_arg("--from", Some(value)) {
                    Ok(dt) => dt,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                },
                None => now,
            };
            let to_dt = match to.as_deref() {
                Some(value) => match parse_required_datetime_arg("--to", Some(value)) {
                    Ok(dt) => dt,
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                },
                None => from_dt + chrono::Duration::hours(24),
            };
            match cueward_adapter_macos::calendar::list_events(from_dt, to_dt, calendar.as_deref()) {
                Ok(events) => {
                    println!("{}", serde_json::to_string_pretty(&events).unwrap());
                    eprintln!("{} event(s)", events.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        CalendarAction::Create {
            title,
            start,
            end,
            calendar,
            notes,
            location,
        } => {
            let start_dt = match parse_datetime_arg("start", &start) {
                Ok(dt) => dt,
                Err(err) => {
                    eprintln!("{err}");
                    process::exit(1);
                }
            };
            let end_dt = match parse_datetime_arg("end", &end) {
                Ok(dt) => dt,
                Err(err) => {
                    eprintln!("{err}");
                    process::exit(1);
                }
            };
            match cueward_adapter_macos::calendar::create_event(
                &title,
                start_dt,
                end_dt,
                calendar.as_deref(),
                notes.as_deref(),
                location.as_deref(),
            ) {
                Ok(()) => eprintln!("event created: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        CalendarAction::Delete {
            title,
            start,
            calendar,
        } => {
            let start_dt = match parse_datetime_arg("start", &start) {
                Ok(dt) => dt,
                Err(err) => {
                    eprintln!("{err}");
                    process::exit(1);
                }
            };
            match cueward_adapter_macos::calendar::delete_event(&title, start_dt, &calendar) {
                Ok(()) => eprintln!("event deleted: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        CalendarAction::Update {
            title,
            calendar,
            new_title,
            new_start,
            new_end,
            notes,
            location,
        } => {
            let new_start_dt = match new_start.as_deref() {
                Some(value) => match parse_datetime_arg("new-start", value) {
                    Ok(dt) => Some(dt),
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                },
                None => None,
            };
            let new_end_dt = match new_end.as_deref() {
                Some(value) => match parse_datetime_arg("new-end", value) {
                    Ok(dt) => Some(dt),
                    Err(err) => {
                        eprintln!("{err}");
                        process::exit(1);
                    }
                },
                None => None,
            };
            match cueward_adapter_macos::calendar::update_event(
                &title,
                calendar.as_deref(),
                new_title.as_deref(),
                new_start_dt,
                new_end_dt,
                notes.as_deref(),
                location.as_deref(),
            ) {
                Ok(()) => eprintln!("event updated: {title}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
