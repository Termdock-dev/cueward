use clap::Parser;

use super::{Cli, Command};
use super::safari::SafariAction;
use super::safari_bookmarks::SafariBookmarksAction;

#[test]
fn cli_parses_safari_bookmarks_list_with_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "list",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks list");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Bookmarks {
                    action: SafariBookmarksAction::List { profile, folder },
                },
        } => {
            assert_eq!(profile, None);
            assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_bookmarks_list_with_profile() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "list",
        "--profile",
        "Work",
    ])
    .expect("parse safari bookmarks list with profile");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Bookmarks {
                    action: SafariBookmarksAction::List { profile, folder },
                },
        } => {
            assert_eq!(profile.as_deref(), Some("Work"));
            assert_eq!(folder, None);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_bookmarks_search_with_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "search",
        "claude",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks search");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Bookmarks {
                    action:
                        SafariBookmarksAction::Search {
                            query,
                            profile,
                            folder,
                        },
                },
        } => {
            assert_eq!(query, "claude");
            assert_eq!(profile, None);
            assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_bookmarks_add_with_title_url_and_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "add",
        "--title",
        "Claude",
        "--url",
        "https://claude.ai",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks add");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Bookmarks {
                    action:
                        SafariBookmarksAction::Add {
                            title,
                            url,
                            profile,
                            folder,
                        },
                },
        } => {
            assert_eq!(title, "Claude");
            assert_eq!(url, "https://claude.ai");
            assert_eq!(profile, None);
            assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_bookmarks_add_with_profile_and_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "add",
        "--title",
        "Claude",
        "--url",
        "https://claude.ai",
        "--profile",
        "Work",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks add with profile");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Bookmarks {
                    action:
                        SafariBookmarksAction::Add {
                            title,
                            url,
                            profile,
                            folder,
                        },
                },
        } => {
            assert_eq!(title, "Claude");
            assert_eq!(url, "https://claude.ai");
            assert_eq!(profile.as_deref(), Some("Work"));
            assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_safari_bookmarks_delete_with_title_url_and_folder() {
    let cli = Cli::try_parse_from([
        "cueward",
        "safari",
        "bookmarks",
        "delete",
        "--title",
        "Claude",
        "--url",
        "https://claude.ai",
        "--folder",
        "Work/AI Tools",
    ])
    .expect("parse safari bookmarks delete");

    match cli.command {
        Command::Safari {
            action:
                SafariAction::Bookmarks {
                    action:
                        SafariBookmarksAction::Delete {
                            title,
                            url,
                            profile,
                            folder,
                        },
                },
        } => {
            assert_eq!(title, "Claude");
            assert_eq!(url, "https://claude.ai");
            assert_eq!(profile, None);
            assert_eq!(folder.as_deref(), Some("Work/AI Tools"));
        }
        _ => panic!("unexpected command"),
    }
}
