use clap::Parser;

use super::stickies::StickiesAction;
use super::{Cli, Command};

#[test]
fn cli_parses_stickies_list() {
    let cli = Cli::try_parse_from(["cueward", "stickies", "list"])
        .expect("parse stickies list");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::List,
        } => {}
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_stickies_create() {
    let cli = Cli::try_parse_from([
        "cueward",
        "stickies",
        "create",
        "--title",
        "臨時待辦",
        "--body",
        "記得回覆客戶",
    ])
    .expect("parse stickies create");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Create { title, body },
        } => {
            assert_eq!(title, "臨時待辦");
            assert_eq!(body, "記得回覆客戶");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_stickies_update() {
    let cli = Cli::try_parse_from([
        "cueward",
        "stickies",
        "update",
        "--id",
        "sticky-1",
        "--title",
        "新標題",
        "--body",
        "更新後內容",
    ])
    .expect("parse stickies update");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Update { id, title, body },
        } => {
            assert_eq!(id, "sticky-1");
            assert_eq!(title.as_deref(), Some("新標題"));
            assert_eq!(body.as_deref(), Some("更新後內容"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_stickies_delete() {
    let cli = Cli::try_parse_from([
        "cueward",
        "stickies",
        "delete",
        "--id",
        "sticky-1",
    ])
    .expect("parse stickies delete");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Delete { id },
        } => assert_eq!(id, "sticky-1"),
        _ => panic!("unexpected command"),
    }
}
