use std::process;

use crate::commands::helpers::print_external;
use crate::commands::safari_ai::dispatch as dispatch_ai;
use crate::commands::safari_bookmarks::dispatch as dispatch_bookmarks;

use super::{SafariAction, build_wait_condition};

pub(crate) fn dispatch(action: SafariAction) {
    match action {
        SafariAction::Tabs { profile } => {
            match cueward_adapter_macos::safari::tabs(profile.as_deref()) {
                Ok(tabs) => {
                    println!("{}", serde_json::to_string_pretty(&tabs).unwrap());
                    eprintln!("{} tab(s)", tabs.len());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
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
            match cueward_adapter_macos::safari::scroll(
                &direction,
                amount,
                profile.as_deref(),
                tab.as_deref(),
            ) {
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
            match cueward_adapter_macos::safari::scroll_and_read(
                times,
                amount,
                selector.as_deref(),
                profile.as_deref(),
                tab.as_deref(),
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
                        serde_json::to_string_pretty(&serde_json::json!({ "closed": count }))
                            .unwrap()
                    );
                    eprintln!("{count} tab(s) closed");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Read {
            selector,
            profile,
            tab,
        } => {
            match cueward_adapter_macos::safari::read(
                selector.as_deref(),
                profile.as_deref(),
                tab.as_deref(),
            ) {
                Ok(result) => {
                    print_external(
                        "safari/read",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("read page content");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Source { profile, tab } => {
            match cueward_adapter_macos::safari::source(profile.as_deref(), tab.as_deref()) {
                Ok(result) => {
                    print_external(
                        "safari/source",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("read page source");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Exec {
            js_code,
            profile,
            tab,
            timeout,
            body,
        } => {
            match cueward_adapter_macos::safari::exec(
                &js_code,
                profile.as_deref(),
                tab.as_deref(),
                timeout,
                body,
            ) {
                Ok(result) => {
                    print_external(
                        "safari/exec",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    eprintln!("executed javascript");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Click {
            selector,
            profile,
            tab,
        } => {
            match cueward_adapter_macos::safari::click(
                &selector,
                profile.as_deref(),
                tab.as_deref(),
            ) {
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
        SafariAction::Fill {
            selector,
            text,
            profile,
            tab,
        } => {
            match cueward_adapter_macos::safari::fill(
                &selector,
                &text,
                profile.as_deref(),
                tab.as_deref(),
            ) {
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
        SafariAction::Wait {
            selector,
            absent,
            text,
            js,
            url,
            navigation,
            timeout,
            profile,
            tab,
        } => {
            let condition = match build_wait_condition(selector, absent, text, js, url, navigation)
            {
                Ok(condition) => condition,
                Err(error) => {
                    eprintln!("error: {error}");
                    process::exit(1);
                }
            };
            match cueward_adapter_macos::safari::wait_until(
                condition,
                timeout,
                profile.as_deref(),
                tab.as_deref(),
            ) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                    eprintln!("wait condition matched");
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Inspect {
            limit,
            profile,
            tab,
        } => {
            match cueward_adapter_macos::safari::inspect(limit, profile.as_deref(), tab.as_deref())
            {
                Ok(result) => {
                    print_external(
                        "safari/inspect",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    process::exit(1);
                }
            }
        }
        SafariAction::Batch {
            steps,
            profile,
            tab,
        } => {
            let steps = match serde_json::from_str(&steps) {
                Ok(steps) => steps,
                Err(error) => {
                    eprintln!("error: invalid batch JSON: {error}");
                    process::exit(1);
                }
            };
            match cueward_adapter_macos::safari::batch(&steps, profile.as_deref(), tab.as_deref()) {
                Ok(result) => {
                    print_external(
                        "safari/batch",
                        &serde_json::to_string_pretty(&result).unwrap(),
                    );
                    if result.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
                        process::exit(1);
                    }
                }
                Err(error) => {
                    eprintln!("error: {error}");
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
