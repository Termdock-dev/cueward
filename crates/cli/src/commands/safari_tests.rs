use clap::Parser;

use super::safari::SafariAction;
use super::{Cli, Command};

#[test]
fn cli_parses_safari_exec_with_profile() {
    let cli = Cli::try_parse_from(["cueward", "safari", "exec", "--profile", "Work", "1+1"])
        .expect("parse safari exec with profile");

    match cli.command {
        Command::Safari {
            action: SafariAction::Exec {
                js_code, profile, ..
            },
        } => {
            assert_eq!(js_code, "1+1");
            assert_eq!(profile.as_deref(), Some("Work"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_exec_timeout() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "exec",
        "--timeout",
        "45",
        "await Promise.resolve([1, 2])",
    ])
    .expect("parse Safari exec timeout");

    match cli.command {
        Command::Safari {
            action: SafariAction::Exec {
                js_code, timeout, ..
            },
        } => {
            assert_eq!(js_code, "await Promise.resolve([1, 2])");
            assert_eq!(timeout, 45);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_active_with_profile() {
    let cli = Cli::try_parse_from(["cueward", "safari", "active", "--profile", "Work"])
        .expect("parse safari active with profile");

    match cli.command {
        Command::Safari {
            action: SafariAction::Active { profile },
        } => assert_eq!(profile.as_deref(), Some("Work")),
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_scroll_and_read() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "scroll-and-read",
        "--tab",
        "x.com",
        "--profile",
        "Work",
        "--times",
        "3",
    ])
    .expect("parse scroll-and-read");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::ScrollAndRead {
                    tab,
                    profile,
                    times,
                    amount,
                    selector,
                },
        } => {
            assert_eq!(tab.as_deref(), Some("x.com"));
            assert_eq!(profile.as_deref(), Some("Work"));
            assert_eq!(times, 3);
            assert_eq!(amount, None);
            assert_eq!(selector, None);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_wait_for_absence_in_selected_tab() {
    let cli = Cli::try_parse_from([
        "cueward", "safari", "wait", "#loading", "--absent", "--tab", "Docs",
    ])
    .expect("parse wait for absence");

    match cli.command {
        Command::Safari {
            action: SafariAction::Wait {
                selector,
                absent,
                tab,
                ..
            },
        } => {
            assert_eq!(selector.as_deref(), Some("#loading"));
            assert!(absent);
            assert_eq!(tab.as_deref(), Some("Docs"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_inspect_and_batch() {
    let inspect = Cli::try_parse_from([
        "cueward", "safari", "inspect", "--tab", "Docs", "--limit", "50",
    ])
    .expect("parse inspect");
    assert!(matches!(
        inspect.command,
        Command::Safari {
            action: SafariAction::Inspect { limit: 50, .. }
        }
    ));

    let batch = Cli::try_parse_from([
        "cueward",
        "safari",
        "batch",
        "--steps",
        "[{\"action\":\"click\",\"ref\":\"1:2\"}]",
    ])
    .expect("parse batch");
    assert!(matches!(
        batch.command,
        Command::Safari {
            action: SafariAction::Batch { .. }
        }
    ));
}
