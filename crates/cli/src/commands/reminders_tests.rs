use clap::Parser;

use super::reminders::RemindersAction;
use super::{Cli, Command};

#[test]
fn cli_parses_reminders_create_with_due_and_priority() {
    let cli = Cli::try_parse_from([
        "cueward",
        "reminders",
        "create",
        "--title",
        "顧問課綱",
        "--due",
        "2026-04-15",
        "--list",
        "提醒事項",
        "--notes",
        "準備報價單",
        "--priority",
        "5",
    ])
    .expect("parse reminders create");

    match cli.command {
        Command::Reminders {
            action:
                RemindersAction::Create {
                    title,
                    due,
                    list,
                    notes,
                    priority,
                },
        } => {
            assert_eq!(title, "顧問課綱");
            assert_eq!(due.as_deref(), Some("2026-04-15"));
            assert_eq!(list, "提醒事項");
            assert_eq!(notes.as_deref(), Some("準備報價單"));
            assert_eq!(priority, Some(5));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reminders_complete_by_id() {
    let cli = Cli::try_parse_from([
        "cueward",
        "reminders",
        "complete",
        "--id",
        "reminder-id-123",
    ])
    .expect("parse reminders complete");

    match cli.command {
        Command::Reminders {
            action: RemindersAction::Complete { title, id },
        } => {
            assert_eq!(title, None);
            assert_eq!(id.as_deref(), Some("reminder-id-123"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reminders_update_by_title() {
    let cli = Cli::try_parse_from([
        "cueward",
        "reminders",
        "update",
        "--title",
        "顧問課綱",
        "--due",
        "2026-04-16",
        "--notes",
        "改日期",
        "--new-title",
        "顧問課綱-新版",
    ])
    .expect("parse reminders update");

    match cli.command {
        Command::Reminders {
            action:
                RemindersAction::Update {
                    title,
                    id,
                    due,
                    notes,
                    new_title,
                    list,
                    priority,
                },
        } => {
            assert_eq!(title.as_deref(), Some("顧問課綱"));
            assert_eq!(id, None);
            assert_eq!(due.as_deref(), Some("2026-04-16"));
            assert_eq!(notes.as_deref(), Some("改日期"));
            assert_eq!(new_title.as_deref(), Some("顧問課綱-新版"));
            assert_eq!(list, None);
            assert_eq!(priority, None);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reminders_list_with_due_before() {
    let cli = Cli::try_parse_from([
        "cueward",
        "reminders",
        "list",
        "--list",
        "提醒事項",
        "--due-before",
        "2026-04-16",
    ])
    .expect("parse reminders list");

    match cli.command {
        Command::Reminders {
            action:
                RemindersAction::List {
                    list,
                    due_before,
                    due_tomorrow,
                },
        } => {
            assert_eq!(list.as_deref(), Some("提醒事項"));
            assert_eq!(due_before.as_deref(), Some("2026-04-16"));
            assert!(!due_tomorrow);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_reminders_list_with_due_tomorrow() {
    let cli = Cli::try_parse_from(["cueward", "reminders", "list", "--due-tomorrow"])
        .expect("parse reminders list tomorrow");

    match cli.command {
        Command::Reminders {
            action:
                RemindersAction::List {
                    list,
                    due_before,
                    due_tomorrow,
                },
        } => {
            assert_eq!(list, None);
            assert_eq!(due_before, None);
            assert!(due_tomorrow);
        }
        _ => panic!("unexpected command"),
    }
}
