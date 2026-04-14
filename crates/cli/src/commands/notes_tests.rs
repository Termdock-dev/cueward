use clap::Parser;

use super::notes::NotesAction;
use super::quick_notes::QuickNotesAction;
use super::{Cli, Command};

#[test]
fn cli_parses_send_with_body_folder_and_notify() {
    let cli = Cli::try_parse_from([
        "cueward",
        "send",
        "--title",
        "Daily Digest",
        "--body",
        "Line 1\nLine 2",
        "--folder",
        "Cueward",
        "--notify",
    ])
    .expect("parse send command");

    match cli.command {
        Command::Send {
            title,
            body,
            folder,
            notify,
        } => {
            assert_eq!(title, "Daily Digest");
            assert_eq!(body.as_deref(), Some("Line 1\nLine 2"));
            assert_eq!(folder, "Cueward");
            assert!(notify);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_notes_update() {
    let cli = Cli::try_parse_from([
        "cueward",
        "notes",
        "update",
        "--title",
        "Draft",
        "--body",
        "Updated body",
        "--folder",
        "Cueward",
    ])
    .expect("parse notes update");

    match cli.command {
        Command::Notes {
            action:
                NotesAction::Update {
                    title,
                    body,
                    folder,
                },
        } => {
            assert_eq!(title, "Draft");
            assert_eq!(body, "Updated body");
            assert_eq!(folder, "Cueward");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_notes_create() {
    let cli = Cli::try_parse_from([
        "cueward",
        "notes",
        "create",
        "--title",
        "會議筆記",
        "--body",
        "內容...",
        "--folder",
        "工作",
    ])
    .expect("parse notes create");

    match cli.command {
        Command::Notes {
            action:
                NotesAction::Create {
                    title,
                    body,
                    folder,
                },
        } => {
            assert_eq!(title, "會議筆記");
            assert_eq!(body, "內容...");
            assert_eq!(folder, "工作");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_notes_delete() {
    let cli = Cli::try_parse_from([
        "cueward",
        "notes",
        "delete",
        "--title",
        "Draft",
        "--folder",
        "Cueward",
    ])
    .expect("parse notes delete");

    match cli.command {
        Command::Notes {
            action: NotesAction::Delete { title, folder },
        } => {
            assert_eq!(title, "Draft");
            assert_eq!(folder, "Cueward");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_notes_move() {
    let cli = Cli::try_parse_from([
        "cueward",
        "notes",
        "move",
        "--title",
        "Draft",
        "--from",
        "Cueward",
        "--to",
        "Archive",
    ])
    .expect("parse notes move");

    match cli.command {
        Command::Notes {
            action:
                NotesAction::Move {
                    title,
                    from,
                    to,
                },
        } => {
            assert_eq!(title, "Draft");
            assert_eq!(from, "Cueward");
            assert_eq!(to, "Archive");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_quick_notes_list() {
    let cli = Cli::try_parse_from(["cueward", "quick-notes", "list"])
        .expect("parse quick-notes list");

    match cli.command {
        Command::QuickNotes {
            action: QuickNotesAction::List,
        } => {}
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_quick_notes_create() {
    let cli = Cli::try_parse_from([
        "cueward",
        "quick-notes",
        "create",
        "--title",
        "Inbox",
        "--body",
        "Capture this",
    ])
    .expect("parse quick-notes create");

    match cli.command {
        Command::QuickNotes {
            action: QuickNotesAction::Create { title, body },
        } => {
            assert_eq!(title, "Inbox");
            assert_eq!(body, "Capture this");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_quick_notes_archive() {
    let cli = Cli::try_parse_from([
        "cueward",
        "quick-notes",
        "archive",
        "--title",
        "Inbox",
        "--to",
        "Archive",
    ])
    .expect("parse quick-notes archive");

    match cli.command {
        Command::QuickNotes {
            action: QuickNotesAction::Archive { title, to },
        } => {
            assert_eq!(title, "Inbox");
            assert_eq!(to, "Archive");
        }
        _ => panic!("unexpected command"),
    }
}
