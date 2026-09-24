use serde_json::Value;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::target::{execute_js_in_tab, resolve_tab};

const BOOTSTRAP: &str = r#"
  if (!window.__cuewardObserve || window.__cuewardObserve.version !== 1) {
    const state = {
      version: 1, started_at: new Date().toISOString(), sequence: 0,
      console: [], network: [], console_dropped: 0, network_dropped: 0
    };
    window.__cuewardObserve = state;
    const append = (kind, record) => {
      const list = state[kind];
      list.push(record);
      if (list.length > 300) {
        list.shift();
        state[kind + '_dropped']++;
      }
    };
    const preview = value => {
      try {
        if (value instanceof Error) return `${value.name}: ${value.message}\n${value.stack || ''}`.slice(0, 4096);
        if (typeof value === 'string') return value.slice(0, 4096);
        const encoded = JSON.stringify(value);
        return (encoded === undefined ? String(value) : encoded).slice(0, 4096);
      } catch (_) { return String(value).slice(0, 4096); }
    };
    for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
      const original = console[level];
      if (typeof original !== 'function') continue;
      console[level] = function(...args) {
        append('console', {
          id: ++state.sequence, level, timestamp: new Date().toISOString(),
          text: args.map(preview).join(' ')
        });
        return original.apply(this, args);
      };
    }
    if (typeof window.fetch === 'function') {
      const originalFetch = window.fetch;
      window.fetch = function(input, init) {
        const id = ++state.sequence;
        const started = performance.now();
        const record = {
          id, type: 'fetch', timestamp: new Date().toISOString(),
          url: String(input?.url || input),
          method: String(init?.method || input?.method || 'GET').toUpperCase(),
          state: 'pending'
        };
        append('network', record);
        let promise;
        try { promise = originalFetch.apply(this, arguments); }
        catch (error) {
          Object.assign(record, {state: 'failed', duration_ms: Math.round(performance.now() - started), error: String(error)});
          throw error;
        }
        return Promise.resolve(promise).then(response => {
          Object.assign(record, {
            state: 'complete', status: response.status, status_text: response.statusText,
            duration_ms: Math.round(performance.now() - started),
            response_headers: Object.fromEntries(response.headers?.entries?.() || [])
          });
          try {
            const type = response.headers?.get?.('content-type') || '';
            if (/text|json|xml|javascript/i.test(type)) {
              response.clone().text().then(body => { record.response_preview = body.slice(0, 16384); }).catch(() => {});
            }
          } catch (_) { /* response body may be inaccessible */ }
          return response;
        }, error => {
          Object.assign(record, {state: 'failed', duration_ms: Math.round(performance.now() - started), error: String(error)});
          throw error;
        });
      };
    }
    if (typeof XMLHttpRequest !== 'undefined') {
      const metadata = new WeakMap();
      const originalOpen = XMLHttpRequest.prototype.open;
      const originalHeader = XMLHttpRequest.prototype.setRequestHeader;
      const originalSend = XMLHttpRequest.prototype.send;
      XMLHttpRequest.prototype.open = function(method, url) {
        metadata.set(this, {method: String(method).toUpperCase(), url: String(url), request_headers: {}});
        return originalOpen.apply(this, arguments);
      };
      XMLHttpRequest.prototype.setRequestHeader = function(name, value) {
        const meta = metadata.get(this);
        if (meta) meta.request_headers[String(name)] = String(value);
        return originalHeader.apply(this, arguments);
      };
      XMLHttpRequest.prototype.send = function() {
        const meta = metadata.get(this) || {method: 'GET', url: '', request_headers: {}};
        const started = performance.now();
        const record = {
          id: ++state.sequence, type: 'xhr', timestamp: new Date().toISOString(),
          url: meta.url, method: meta.method, request_headers: meta.request_headers,
          state: 'pending'
        };
        append('network', record);
        this.addEventListener('loadend', () => {
          Object.assign(record, {
            state: this.status ? 'complete' : 'failed', status: this.status,
            status_text: this.statusText,
            duration_ms: Math.round(performance.now() - started),
            response_headers: this.getAllResponseHeaders()
          });
          try {
            if (!this.responseType || this.responseType === 'text')
              record.response_preview = this.responseText.slice(0, 16384);
          } catch (_) { /* binary response */ }
        }, {once: true});
        try { return originalSend.apply(this, arguments); }
        catch (error) {
          Object.assign(record, {state: 'failed', duration_ms: Math.round(performance.now() - started), error: String(error)});
          throw error;
        }
      };
    }
  }
"#;

fn read_observation(
    query: &str,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<Value, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let script = format!(
            "(() => {{ {BOOTSTRAP} const state = window.__cuewardObserve; return JSON.stringify({query}); }})()"
        );
        let payload = execute_js_in_tab(&script, &tab, "safari_observe")?;
        serde_json::from_str(&payload)
            .map_err(|error| MacosError::Other(format!("invalid Safari observation: {error}")))
    })
}

pub fn console_messages(
    level: Option<&str>,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<Value, MacosError> {
    if let Some(level) = level {
        if !["log", "info", "warn", "error", "debug"].contains(&level) {
            return Err(MacosError::Other(format!(
                "unsupported console level: {level}"
            )));
        }
    }
    let filter = serde_json::to_string(&level)
        .map_err(|error| MacosError::Other(format!("invalid console level: {error}")))?;
    read_observation(
        &format!(
            "{{started_at: state.started_at, dropped: state.console_dropped, messages: state.console.filter(item => {filter} === null || item.level === {filter})}}"
        ),
        profile_filter,
        tab_selector,
    )
}

pub fn network_requests(
    id: Option<u64>,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
) -> Result<Value, MacosError> {
    let query = match id {
        Some(id) => format!("{{started_at: state.started_at, request: state.network.find(item => item.id === {id}) || null}}"),
        None => "{started_at: state.started_at, dropped: state.network_dropped, requests: state.network.map(({id, type, timestamp, url, method, state: status, status: http_status, duration_ms, error}) => ({id, type, timestamp, url, method, state: status, status: http_status, duration_ms, error}))}".to_string(),
    };
    let result = read_observation(&query, profile_filter, tab_selector)?;
    if id.is_some() && result.get("request").is_some_and(Value::is_null) {
        return Err(MacosError::Other(format!(
            "network request {} not found in current page capture",
            id.unwrap_or_default()
        )));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::BOOTSTRAP;

    #[test]
    fn capture_installs_once_and_records_console_and_fetch() {
        let script = format!(
            r#"
            globalThis.window = globalThis;
            globalThis.fetch = async () => ({{
              status: 201, statusText: 'Created',
              headers: new Map([['content-type', 'text/plain']]),
              clone: () => ({{text: async () => 'captured'}})
            }});
            (() => {{ {BOOTSTRAP} }})();
            (() => {{ {BOOTSTRAP} }})();
            console.error('observed');
            fetch('/test').then(() => setImmediate(() => process.stdout.write(JSON.stringify({{
              messages: window.__cuewardObserve.console,
              requests: window.__cuewardObserve.network
            }}))));
            "#
        );
        let output = Command::new("node")
            .arg("-e")
            .arg(script)
            .output()
            .expect("Node.js is required for Safari JavaScript behavior tests");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("capture JSON");
        assert_eq!(result["messages"].as_array().unwrap().len(), 1);
        assert_eq!(result["messages"][0]["level"], "error");
        assert_eq!(result["requests"][0]["status"], 201);
        assert_eq!(result["requests"][0]["response_preview"], "captured");
    }
}
