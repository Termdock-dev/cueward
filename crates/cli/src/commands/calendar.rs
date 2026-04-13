use std::process;

use clap::Subcommand;
use chrono::Local;

use super::helpers::{local_day_bounds, parse_datetime_arg, parse_required_datetime_arg};

#[derive(Subcommand)]
pub(crate) enum CalendarAction {
    List {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        calendar: Option<String>,
    },
    Today {
        #[arg(long)]
        calendar: Option<String>,
    },
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: String,
        #[arg(long)]
        calendar: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        location: Option<String>,
    },
    Delete {
        #[arg(long)]
        title: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        calendar: String,
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
    }
}
