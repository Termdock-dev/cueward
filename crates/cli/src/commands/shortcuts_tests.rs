use clap::Parser;

use crate::commands::{Cli, Command};

#[test]
fn cli_parses_shortcuts_create() {
    let cli = Cli::try_parse_from(["cueward", "shortcuts", "create", "Clean URL Share"]).unwrap();

    match cli.command {
        Command::Shortcuts { .. } => {}
        _ => panic!("expected shortcuts command"),
    }
}

#[test]
fn cli_parses_shortcuts_apply() {
    let cli = Cli::try_parse_from(["cueward", "shortcuts", "apply", "clean-url-share.yaml"]).unwrap();

    match cli.command {
        Command::Shortcuts { .. } => {}
        _ => panic!("expected shortcuts command"),
    }
}

#[test]
fn cli_parses_shortcuts_add_replace_text() {
    let cli = Cli::try_parse_from([
        "cueward",
        "shortcuts",
        "add-replace-text",
        "--name",
        "Clean URL Share",
        "--from",
        "input_url_text",
        "--find",
        "foo",
        "--replace",
        "bar",
        "--regex",
        "--ignore-case",
        "--output",
        "tracking_removed",
    ])
    .unwrap();

    match cli.command {
        Command::Shortcuts { .. } => {}
        _ => panic!("expected shortcuts command"),
    }
}

#[test]
fn cli_parses_shortcuts_add_get_urls() {
    let cli = Cli::try_parse_from([
        "cueward",
        "shortcuts",
        "add-get-urls",
        "--name",
        "Clean URL Share",
        "--from",
        "input_url_text",
        "--output",
        "urls",
    ])
    .unwrap();

    match cli.command {
        Command::Shortcuts { .. } => {}
        _ => panic!("expected shortcuts command"),
    }
}

#[test]
fn cli_parses_shortcuts_move() {
    let cli = Cli::try_parse_from([
        "cueward",
        "shortcuts",
        "move",
        "--name",
        "Clean URL Share",
        "Work",
    ])
    .unwrap();

    match cli.command {
        Command::Shortcuts { .. } => {}
        _ => panic!("expected shortcuts command"),
    }
}

#[test]
fn cli_parses_shortcuts_export_spec() {
    let cli = Cli::try_parse_from([
        "cueward",
        "shortcuts",
        "export-spec",
        "--name",
        "Clean URL Share",
    ])
    .unwrap();

    match cli.command {
        Command::Shortcuts { .. } => {}
        _ => panic!("expected shortcuts command"),
    }
}

#[test]
fn cli_rejects_shortcuts_selector_with_id_and_name() {
    let result = Cli::try_parse_from([
        "cueward",
        "shortcuts",
        "run",
        "--id",
        "wf-1",
        "--name",
        "Clean URL Share",
    ]);

    let err = match result {
        Ok(_) => panic!("expected parse failure"),
        Err(err) => err,
    };
    let rendered = err.to_string();
    assert!(rendered.contains("--id"));
    assert!(rendered.contains("--name"));
}

#[test]
fn cli_rejects_shortcuts_run_without_selector() {
    let result = Cli::try_parse_from(["cueward", "shortcuts", "run"]);

    let err = match result {
        Ok(_) => panic!("expected parse failure"),
        Err(err) => err,
    };
    let rendered = err.to_string();
    assert!(rendered.contains("--id"));
    assert!(rendered.contains("--name"));
}
