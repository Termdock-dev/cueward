use std::process;

use clap::Subcommand;

use super::safari_ai::{SafariAiAction, SafariAiProvider, dispatch as dispatch_ai};
use super::safari_bookmarks::{SafariBookmarksAction, dispatch as dispatch_bookmarks};
use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum SafariAction {
    Tabs {
        #[arg(long)]
        profile: Option<String>,
    },
    Active {
        #[arg(long)]
        profile: Option<String>,
    },
    Open {
        url: String,
        #[arg(long)]
        profile: Option<String>,
    },
    Close {
        #[arg(long)]
        index: Option<usize>,
    },
    Scroll {
        direction: String,
        #[arg(long)]
        amount: Option<i64>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        tab: Option<String>,
    },
    ScrollAndRead {
        #[arg(long, default_value = "1")]
        times: u64,
        #[arg(long)]
        amount: Option<i64>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        tab: Option<String>,
        #[arg(long)]
        selector: Option<String>,
    },
    CloseTabs {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        url: Option<String>,
    },
    Read {
        #[arg(long)]
        selector: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        tab: Option<String>,
    },
    Source {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        tab: Option<String>,
    },
    Exec {
        js_code: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        tab: Option<String>,
    },
    Click {
        selector: String,
    },
    Fill {
        selector: String,
        text: String,
    },
    Wait {
        selector: String,
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    Bookmarks {
        #[command(subcommand)]
        action: SafariBookmarksAction,
    },
    Ai {
        #[arg(long)]
        provider: SafariAiProvider,
        #[arg(long)]
        profile: Option<String>,
        #[command(subcommand)]
        action: SafariAiAction,
    },
}

pub(crate) fn dispatch(action: SafariAction) {
    match action {
        SafariAction::Tabs { profile } => match cueward_adapter_macos::safari::tabs(profile.as_deref()) {
            Ok(tabs) => {
                println!("{}", serde_json::to_string_pretty(&tabs).unwrap());
                eprintln!("{} tab(s)", tabs.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        SafariAction::Active { profile } => {
            match cueward_adapter_macos::safari::active(profile.as_deref()) {
                Ok(tab) => {
                    println!("{}", serde_json::to_string_pretty(&tab).unwrap());
                    if tab.is_some() {
                        eprintln!("active tab");
                    } else {
                        eprintln!("no Safari window");
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Open { url, profile } => {
            match cueward_adapter_macos::safari::open(&url, profile.as_deref()) {
                Ok(tab) => {
                    println!("{}", serde_json::to_string_pretty(&tab).unwrap());
                    if tab.is_some() {
                        eprintln!("opened tab");
                    } else {
                        eprintln!("no Safari window");
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Close { index } => match cueward_adapter_macos::safari::close(index) {
            Ok(result) => {
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                if result.closed {
                    eprintln!("tab closed");
                } else {
                    eprintln!("no Safari window");
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        SafariAction::Scroll {
            direction,
            amount,
            profile,
            tab,
        } => {
            if let Some(ref t) = tab {
                if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref()) {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            match cueward_adapter_macos::safari::scroll(&direction, amount, profile.as_deref()) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("scrolled {direction}");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::ScrollAndRead {
            times,
            amount,
            profile,
            tab,
            selector,
        } => {
            if let Some(ref t) = tab {
                if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref()) {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            match cueward_adapter_macos::safari::scroll_and_read(
                times,
                amount,
                selector.as_deref(),
                profile.as_deref(),
            ) {
                Ok(result) => {
                    print_external(
                        "safari/scroll-and-read",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("scroll/read pipeline complete");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::CloseTabs { profile, url } => {
            match cueward_adapter_macos::safari::close_tabs(profile.as_deref(), url.as_deref()) {
                Ok(count) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({ "closed": count })).unwrap()
                    );
                    eprintln!("{count} tab(s) closed");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Read { selector, profile, tab } => {
            if let Some(ref t) = tab {
                if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref()) {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            match cueward_adapter_macos::safari::read(selector.as_deref(), profile.as_deref()) {
                Ok(result) => {
                    print_external("safari/read", &serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("read page content");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Source { profile, tab } => {
            if let Some(ref t) = tab {
                if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref()) {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            match cueward_adapter_macos::safari::source(profile.as_deref()) {
                Ok(result) => {
                    print_external("safari/source", &serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("read page source");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Exec { js_code, profile, tab } => {
            if let Some(ref t) = tab {
                if let Err(e) = cueward_adapter_macos::safari::focus_tab(t, profile.as_deref()) {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
            match cueward_adapter_macos::safari::exec(&js_code, profile.as_deref()) {
                Ok(result) => {
                    print_external("safari/exec", &serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("executed javascript");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Click { selector } => {
            match cueward_adapter_macos::safari::click(&selector) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("clicked element");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Fill { selector, text } => {
            match cueward_adapter_macos::safari::fill(&selector, &text) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("filled element");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Wait { selector, timeout } => {
            match cueward_adapter_macos::safari::wait(&selector, timeout) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("selector found");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Bookmarks { action } => dispatch_bookmarks(action),
        SafariAction::Ai {
            provider,
            profile,
            action,
        } => dispatch_ai(provider, profile, action),
    }
}
