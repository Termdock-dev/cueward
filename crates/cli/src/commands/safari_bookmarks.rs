use std::process;

use clap::Subcommand;

use super::helpers::{bookmarks_target_folder, print_external};

#[derive(Subcommand)]
pub(crate) enum SafariBookmarksAction {
    List {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        folder: Option<String>,
    },
    Search {
        query: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        folder: Option<String>,
    },
    Add {
        #[arg(long)]
        title: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        folder: Option<String>,
    },
    Delete {
        #[arg(long)]
        title: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        folder: Option<String>,
    },
}

pub(crate) fn dispatch(action: SafariBookmarksAction) {
    match action {
        SafariBookmarksAction::List { profile, folder } => {
            let target_folder = bookmarks_target_folder(profile.as_deref(), folder.as_deref());
            match cueward_adapter_macos::bookmarks::list_bookmarks(target_folder.as_deref()) {
                Ok(result) => {
                    print_external(
                        "safari/bookmarks/list",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("listed bookmarks");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariBookmarksAction::Search {
            query,
            profile,
            folder,
        } => {
            let target_folder = bookmarks_target_folder(profile.as_deref(), folder.as_deref());
            match cueward_adapter_macos::bookmarks::search_bookmarks(&query, target_folder.as_deref()) {
                Ok(result) => {
                    print_external(
                        "safari/bookmarks/search",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("searched bookmarks");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariBookmarksAction::Add {
            title,
            url,
            profile,
            folder,
        } => {
            let target_folder = bookmarks_target_folder(profile.as_deref(), folder.as_deref());
            match cueward_adapter_macos::bookmarks::add_bookmark_cli(
                &title,
                &url,
                target_folder.as_deref(),
            ) {
                Ok(result) => {
                    print_external(
                        "safari/bookmarks/add",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("bookmark added");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariBookmarksAction::Delete {
            title,
            url,
            profile,
            folder,
        } => {
            let target_folder = bookmarks_target_folder(profile.as_deref(), folder.as_deref());
            match cueward_adapter_macos::bookmarks::delete_bookmark_cli(
                &title,
                &url,
                target_folder.as_deref(),
            ) {
                Ok(result) => {
                    print_external(
                        "safari/bookmarks/delete",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("bookmark deleted");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
