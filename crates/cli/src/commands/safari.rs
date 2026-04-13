use std::process;

use clap::Subcommand;

use super::safari_ai::{SafariAiAction, SafariAiProvider, dispatch as dispatch_ai};
use super::safari_bookmarks::{SafariBookmarksAction, dispatch as dispatch_bookmarks};
use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum SafariAction {
    /// List all current Safari tabs
    Tabs {
        /// Filter by Safari profile name parsed from window title
        #[arg(long)]
        profile: Option<String>,
    },
    /// Show the current active tab in the front Safari window
    Active {
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
    },
    /// Open a URL in a new Safari tab
    Open {
        /// URL to open
        url: String,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
    },
    /// Close a tab in the front Safari window
    Close {
        /// Zero-based tab index in the front window. Defaults to the current tab.
        #[arg(long)]
        index: Option<usize>,
    },
    /// Scroll the current page
    Scroll {
        /// Direction: up, down, top, bottom
        direction: String,
        /// Pixels to scroll (default 500, ignored for top/bottom)
        #[arg(long)]
        amount: Option<i64>,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },
    /// Scroll repeatedly and return only newly loaded content
    ScrollAndRead {
        /// Number of scroll/read iterations
        #[arg(long, default_value = "1")]
        times: u64,
        /// Pixels to scroll each iteration
        #[arg(long)]
        amount: Option<i64>,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
        /// Optional CSS selector to scope the read area
        #[arg(long)]
        selector: Option<String>,
    },
    /// Close multiple tabs, optionally filtered by profile and/or URL pattern
    CloseTabs {
        /// Restrict to a Safari profile name
        #[arg(long)]
        profile: Option<String>,
        /// Only close tabs whose URL contains this string
        #[arg(long)]
        url: Option<String>,
    },
    /// Read page content from the current active tab
    Read {
        /// Optional CSS selector to extract a specific element's text
        #[arg(long)]
        selector: Option<String>,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },
    /// Read the full HTML source of the current active tab
    Source {
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },
    /// Execute JavaScript in the current active tab
    Exec {
        /// JavaScript code to execute
        js_code: String,
        /// Restrict operations to a Safari profile name parsed from the window title
        #[arg(long)]
        profile: Option<String>,
        /// Target a specific tab by index or URL/title substring
        #[arg(long)]
        tab: Option<String>,
    },
    /// Click an element in the current active tab
    Click {
        /// CSS selector
        selector: String,
    },
    /// Fill an element in the current active tab
    Fill {
        /// CSS selector
        selector: String,
        /// Text to fill
        text: String,
    },
    /// Wait for an element to appear in the current active tab
    Wait {
        /// CSS selector
        selector: String,
        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// Safari bookmarks workflows
    Bookmarks {
        #[command(subcommand)]
        action: SafariBookmarksAction,
    },
    /// Safari AI provider workflows
    Ai {
        /// AI provider to target
        #[arg(long)]
        provider: SafariAiProvider,
        /// Restrict operations to a Safari profile name parsed from the window title
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
