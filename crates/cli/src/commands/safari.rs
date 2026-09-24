use clap::Subcommand;

use super::safari_ai::{SafariAiAction, SafariAiProvider};
use super::safari_bookmarks::SafariBookmarksAction;
use cueward_adapter_macos::safari::WaitCondition;

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
        /// Maximum time to wait for a Promise result, in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// Click an element in the current active tab
    Click {
        /// CSS selector
        selector: String,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a tab without changing the active tab
        #[arg(long)]
        tab: Option<String>,
    },
    /// Fill an element in the current active tab
    Fill {
        /// CSS selector
        selector: String,
        /// Text to fill
        text: String,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a tab without changing the active tab
        #[arg(long)]
        tab: Option<String>,
    },
    /// Wait for an element to appear in the current active tab
    Wait {
        /// CSS selector to appear
        selector: Option<String>,
        /// Wait for the selector to disappear
        #[arg(long, requires = "selector")]
        absent: bool,
        /// Wait for visible page text
        #[arg(long)]
        text: Option<String>,
        /// Wait for a JavaScript expression to become truthy
        #[arg(long)]
        js: Option<String>,
        /// Wait until the page URL contains this text
        #[arg(long)]
        url: Option<String>,
        /// Wait for the selected tab to navigate and finish loading
        #[arg(long)]
        navigation: bool,
        /// Timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a tab without changing the active tab
        #[arg(long)]
        tab: Option<String>,
    },
    /// Read visible page elements and assign short-lived refs
    Inspect {
        /// Maximum number of nodes to return
        #[arg(long, default_value = "200")]
        limit: usize,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a tab without changing the active tab
        #[arg(long)]
        tab: Option<String>,
    },
    /// Run a JSON array of page actions in one call
    Batch {
        /// JSON steps with action and one of ref, selector, or text
        #[arg(long)]
        steps: String,
        /// Restrict operations to a Safari profile
        #[arg(long)]
        profile: Option<String>,
        /// Target a tab without changing the active tab
        #[arg(long)]
        tab: Option<String>,
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

fn build_wait_condition(
    selector: Option<String>,
    absent: bool,
    text: Option<String>,
    js: Option<String>,
    url: Option<String>,
    navigation: bool,
) -> Result<WaitCondition, &'static str> {
    let count = usize::from(selector.is_some())
        + usize::from(text.is_some())
        + usize::from(js.is_some())
        + usize::from(url.is_some())
        + usize::from(navigation);
    if count != 1 {
        return Err("specify exactly one selector, --text, --js, --url, or --navigation");
    }
    if let Some(selector) = selector {
        return Ok(if absent {
            WaitCondition::SelectorAbsent(selector)
        } else {
            WaitCondition::SelectorPresent(selector)
        });
    }
    if let Some(text) = text {
        return Ok(WaitCondition::Text(text));
    }
    if let Some(js) = js {
        return Ok(WaitCondition::JavaScript(js));
    }
    if let Some(url) = url {
        return Ok(WaitCondition::UrlContains(url));
    }
    Ok(WaitCondition::Navigation)
}

mod dispatch;
pub(crate) use dispatch::dispatch;
