use clap::Parser;

use super::{Cli, Command};
use super::safari::SafariAction;

#[test]
fn cli_parses_safari_exec_with_profile() {
    let cli = Cli::try_parse_from(["cueward", "safari", "exec", "--profile", "Work", "1+1"])
        .expect("parse safari exec with profile");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Exec {
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
