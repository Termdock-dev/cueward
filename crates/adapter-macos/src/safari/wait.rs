use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::target::{execute_js_in_tab, resolve_tab};
use super::types::SafariWaitResult;

/// A condition to poll in one Safari tab.
pub enum WaitCondition {
    SelectorPresent(String),
    SelectorAbsent(String),
    Text(String),
    JavaScript(String),
    UrlContains(String),
    Navigation,
}

#[derive(Deserialize)]
struct NavigationStart {
    url: String,
    origin: f64,
}

impl WaitCondition {
    fn name(&self) -> &'static str {
        match self {
            Self::SelectorPresent(_) => "selector-present",
            Self::SelectorAbsent(_) => "selector-absent",
            Self::Text(_) => "text",
            Self::JavaScript(_) => "javascript",
            Self::UrlContains(_) => "url-contains",
            Self::Navigation => "navigation",
        }
    }

    fn selector(&self) -> Option<&str> {
        match self {
            Self::SelectorPresent(selector) | Self::SelectorAbsent(selector) => Some(selector),
            _ => None,
        }
    }

    fn check_js(&self, navigation_start: Option<&NavigationStart>) -> Result<String, MacosError> {
        let literal = |text: &str| {
            serde_json::to_string(text)
                .map_err(|error| MacosError::Other(format!("invalid wait condition: {error}")))
        };
        let predicate = match self {
            Self::SelectorPresent(selector) => {
                format!("Boolean(document.querySelector({}))", literal(selector)?)
            }
            Self::SelectorAbsent(selector) => {
                format!("!document.querySelector({})", literal(selector)?)
            }
            Self::Text(text) => format!(
                "(document.body?.innerText || '').includes({})",
                literal(text)?
            ),
            Self::JavaScript(code) => format!(
                "(() => {{ const value = ({code}); if (value?.then) throw new Error('wait --js requires a synchronous condition'); return Boolean(value); }})()",
            ),
            Self::UrlContains(url) => {
                format!("location.href.includes({})", literal(url)?)
            }
            Self::Navigation => {
                let start = navigation_start.ok_or_else(|| {
                    MacosError::Other("navigation start state missing".to_string())
                })?;
                format!(
                    "(location.href !== {} || performance.timeOrigin !== {}) && document.readyState === 'complete'",
                    literal(&start.url)?,
                    start.origin
                )
            }
        };
        Ok(format!(
            r#"(() => {{
              try {{ return ({predicate}) ? 'true' : 'false'; }}
              catch (error) {{ return 'error:' + String(error?.message || error); }}
            }})()"#
        ))
    }
}

/// Wait until the requested condition is true in the selected tab.
pub fn wait_until(
    condition: WaitCondition,
    timeout_seconds: u64,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<SafariWaitResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let navigation_start = if matches!(condition, WaitCondition::Navigation) {
            let raw = execute_js_in_tab(
                "JSON.stringify({url: location.href, origin: performance.timeOrigin})",
                &tab,
                "safari_wait_navigation_start",
            )?;
            Some(
                serde_json::from_str::<NavigationStart>(&raw).map_err(|error| {
                    MacosError::Other(format!("invalid navigation start state: {error}"))
                })?,
            )
        } else {
            None
        };
        let check_js = condition.check_js(navigation_start.as_ref())?;
        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        loop {
            let result = execute_js_in_tab(&check_js, &tab, "safari_wait")?;
            match result.as_str() {
                "true" => {
                    return Ok(SafariWaitResult {
                        found: true,
                        selector: condition.selector().map(ToOwned::to_owned),
                        condition: condition.name().to_string(),
                        timeout_seconds,
                    });
                }
                "false" => {}
                error if error.starts_with("error:") => {
                    return Err(MacosError::Other(error.to_string()));
                }
                other => {
                    return Err(MacosError::Other(format!(
                        "invalid Safari wait response: {other}"
                    )));
                }
            }
            if Instant::now() >= deadline {
                return Err(MacosError::Other(format!(
                    "timeout waiting for {} after {timeout_seconds} seconds",
                    condition.name()
                )));
            }
            thread::sleep(Duration::from_millis(250));
        }
    })
}
