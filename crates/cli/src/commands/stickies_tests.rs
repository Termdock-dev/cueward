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
        "--display",
        "2",
        "--color",
        "blue",
        "--x",
        "40",
        "--y",
        "80",
        "--width",
        "420",
        "--height",
        "260",
    ])
    .expect("parse stickies create");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Create {
                title,
                body,
                display,
                color,
                x,
                y,
                width,
                height,
            },
        } => {
            assert_eq!(title, "臨時待辦");
            assert_eq!(body, "記得回覆客戶");
            assert_eq!(display, Some(2));
            assert_eq!(color.as_deref(), Some("blue"));
            assert_eq!(x, Some(40));
            assert_eq!(y, Some(80));
            assert_eq!(width, Some(420));
            assert_eq!(height, Some(260));
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
        "--display",
        "3",
        "--color",
        "gray",
        "--x",
        "10",
        "--y",
        "20",
        "--width",
        "360",
        "--height",
        "220",
    ])
    .expect("parse stickies update");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Update {
                id,
                title,
                body,
                display,
                color,
                x,
                y,
                width,
                height,
            },
        } => {
            assert_eq!(id, "sticky-1");
            assert_eq!(title.as_deref(), Some("新標題"));
            assert_eq!(body.as_deref(), Some("更新後內容"));
            assert_eq!(display, Some(3));
            assert_eq!(color.as_deref(), Some("gray"));
            assert_eq!(x, Some(10));
            assert_eq!(y, Some(20));
            assert_eq!(width, Some(360));
            assert_eq!(height, Some(220));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_rejects_stickies_create_x_without_y() {
    let err = match Cli::try_parse_from([
        "cueward",
        "stickies",
        "create",
        "--title",
        "臨時待辦",
        "--body",
        "記得回覆客戶",
        "--x",
        "40",
    ]) {
        Ok(_) => panic!("create should reject x without y"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("--y"));
}

#[test]
fn cli_rejects_stickies_update_width_without_height() {
    let err = match Cli::try_parse_from([
        "cueward",
        "stickies",
        "update",
        "--id",
        "sticky-1",
        "--width",
        "360",
    ]) {
        Ok(_) => panic!("update should reject width without height"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("--height"));
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
