use clap::Parser;

use super::calendar::CalendarAction;
use super::{Cli, Command};

#[test]
fn cli_parses_calendar_update() {
    let cli = Cli::try_parse_from([
        "cueward",
        "calendar",
        "update",
        "--title",
        "顧問會議",
        "--new-start",
        "2026-04-16 14:00",
        "--new-title",
        "顧問會議（改期）",
        "--calendar",
        "Work",
    ])
    .expect("parse calendar update");

    match cli.command {
        Command::Calendar {
            action:
                CalendarAction::Update {
                    title,
                    calendar,
                    new_title,
                    new_start,
                    new_end,
                    notes,
                    location,
                },
        } => {
            assert_eq!(title, "顧問會議");
            assert_eq!(calendar.as_deref(), Some("Work"));
            assert_eq!(new_title.as_deref(), Some("顧問會議（改期）"));
            assert_eq!(new_start.as_deref(), Some("2026-04-16 14:00"));
            assert_eq!(new_end, None);
            assert_eq!(notes, None);
            assert_eq!(location, None);
        }
        _ => panic!("unexpected command"),
    }
}
