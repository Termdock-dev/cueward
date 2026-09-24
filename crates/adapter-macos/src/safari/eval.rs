use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::MacosError;
use crate::safari_guard::with_safari_session;

use super::target::{execute_js_in_tab, resolve_tab};
use super::types::SafariEvalResult;

static NEXT_EVAL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
struct EvalWire {
    status: String,
    #[serde(default)]
    value_type: Option<String>,
    #[serde(default)]
    result_json: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

fn build_eval_js(code: &str, token: &str) -> Result<String, MacosError> {
    let code = serde_json::to_string(code)
        .map_err(|err| MacosError::Other(format!("invalid JavaScript source: {err}")))?;
    let token = serde_json::to_string(token)
        .map_err(|err| MacosError::Other(format!("invalid evaluation token: {err}")))?;
    Ok(
        r#"(() => {
          const code = __CODE__;
          const token = __TOKEN__;
          const store = window.__cuewardEvalPending ||= Object.create(null);
          const failure = (error) => JSON.stringify({
            status: 'error', error: String(error?.stack || error?.message || error)
          });
          const success = (value) => {
            let valueType = value === null ? 'null' :
              Array.isArray(value) ? 'array' : typeof value;
            if (valueType === 'undefined') return JSON.stringify({
              status: 'complete', value_type: valueType, result_json: 'null'
            });
            if (valueType === 'bigint') return JSON.stringify({
              status: 'complete', value_type: valueType,
              result_json: JSON.stringify(value.toString())
            });
            if (valueType === 'number' && !Number.isFinite(value)) {
              valueType = 'nonfinite-number';
              return JSON.stringify({status: 'complete', value_type: valueType,
                result_json: JSON.stringify(String(value))});
            }
            const encoded = JSON.stringify(value);
            if (encoded === undefined) throw new TypeError('JavaScript result is not JSON serializable');
            return JSON.stringify({status: 'complete', value_type: valueType,
              result_json: encoded});
          };
          let result;
          try {
            result = (0, eval)(code);
          } catch (error) {
            if (!(error instanceof SyntaxError) || !/\bawait\b/.test(code)) return failure(error);
            const AsyncFunction = Object.getPrototypeOf(async function() {}).constructor;
            try {
              result = new AsyncFunction('return (' + code + ')')();
            } catch (expressionError) {
              if (!(expressionError instanceof SyntaxError)) return failure(expressionError);
              try { result = new AsyncFunction(code)(); }
              catch (bodyError) { return failure(bodyError); }
            }
          }
          if (result && typeof result.then === 'function') {
            store[token] = JSON.stringify({status: 'running'});
            Promise.resolve(result).then(
              value => { try { store[token] = success(value); }
                         catch (error) { store[token] = failure(error); } },
              error => { store[token] = failure(error); }
            );
            return store[token];
          }
          try { return success(result); }
          catch (error) { return failure(error); }
        })()"#
            .replace("__CODE__", &code)
            .replace("__TOKEN__", &token),
    )
}

fn build_poll_js(token: &str) -> Result<String, MacosError> {
    let token = serde_json::to_string(token)
        .map_err(|err| MacosError::Other(format!("invalid evaluation token: {err}")))?;
    Ok(format!(
        r#"(() => {{
          const store = window.__cuewardEvalPending;
          const result = store?.[{token}];
          if (!result) return JSON.stringify({{status: 'error', error: 'evaluation state lost'}});
          if (JSON.parse(result).status !== 'running') delete store[{token}];
          return result;
        }})()"#
    ))
}

fn decode_eval_wire(payload: &str) -> Result<Option<SafariEvalResult>, MacosError> {
    let wire: EvalWire = serde_json::from_str(payload)
        .map_err(|err| MacosError::Other(format!("invalid Safari evaluation response: {err}")))?;
    match wire.status.as_str() {
        "running" => Ok(None),
        "error" => Err(MacosError::Other(format!(
            "JavaScript evaluation failed: {}",
            wire.error.unwrap_or_else(|| "unknown error".to_string())
        ))),
        "complete" => {
            let result = wire
                .result_json
                .ok_or_else(|| MacosError::Other("JavaScript result had no value".to_string()))?;
            let result = serde_json::from_str(&result).map_err(|err| {
                MacosError::Other(format!("invalid serialized JavaScript result: {err}"))
            })?;
            let value_type = wire
                .value_type
                .ok_or_else(|| MacosError::Other("JavaScript result had no type".to_string()))?;
            Ok(Some(SafariEvalResult { result, value_type }))
        }
        other => Err(MacosError::Other(format!(
            "unknown JavaScript evaluation status: {other}"
        ))),
    }
}

/// Evaluate JavaScript in the selected Safari tab and preserve JSON result types.
pub fn exec(
    js_code: &str,
    profile_filter: Option<&str>,
    tab_selector: Option<&str>,
    timeout_seconds: u64,
) -> Result<SafariEvalResult, MacosError> {
    with_safari_session(|| {
        let tab = resolve_tab(tab_selector, profile_filter)?;
        let token = format!(
            "{}-{}",
            std::process::id(),
            NEXT_EVAL_ID.fetch_add(1, Ordering::Relaxed)
        );
        let initial = execute_js_in_tab(&build_eval_js(js_code, &token)?, &tab, "safari_exec")?;
        if let Some(result) = decode_eval_wire(&initial)? {
            return Ok(result);
        }
        let poll_js = build_poll_js(&token)?;
        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(250));
            let payload = execute_js_in_tab(&poll_js, &tab, "safari_exec_poll")?;
            if let Some(result) = decode_eval_wire(&payload)? {
                return Ok(result);
            }
        }
        Err(MacosError::Other(format!(
            "timeout waiting for JavaScript result after {timeout_seconds} seconds"
        )))
    })
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::{build_eval_js, build_poll_js, decode_eval_wire};

    fn evaluate_with_node(code: &str) -> Option<super::SafariEvalResult> {
        let initial = build_eval_js(code, "test-token").expect("build evaluation");
        let poll = build_poll_js("test-token").expect("build poll");
        let harness = format!(
            "globalThis.window = globalThis; const initial = {initial}; \
             if (JSON.parse(initial).status === 'running') {{ \
               setImmediate(() => process.stdout.write({poll})); \
             }} else {{ process.stdout.write(initial); }}"
        );
        let output = match Command::new("node").arg("-e").arg(harness).output() {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => panic!("run JavaScript bridge: {error}"),
        };
        assert!(
            output.status.success(),
            "JavaScript bridge failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let payload = String::from_utf8(output.stdout).expect("UTF-8 result");
        Some(
            decode_eval_wire(&payload)
                .expect("decode JavaScript result")
                .expect("completed JavaScript result"),
        )
    }

    #[test]
    fn javascript_bridge_preserves_types_and_resolves_await() {
        for (code, expected_type, expected_value) in [
            ("[1,2]", "array", serde_json::json!([1, 2])),
            ("({a:1})", "object", serde_json::json!({"a": 1})),
            ("null", "null", serde_json::Value::Null),
            ("undefined", "undefined", serde_json::Value::Null),
            (
                "await Promise.resolve(42)",
                "number",
                serde_json::json!(42),
            ),
        ] {
            let Some(result) = evaluate_with_node(code) else {
                return;
            };
            assert_eq!(result.value_type, expected_type, "source: {code}");
            assert_eq!(result.result, expected_value, "source: {code}");
        }
    }

    #[test]
    fn decodes_typed_values() {
        let array =
            decode_eval_wire(r#"{"status":"complete","value_type":"array","result_json":"[1,2]"}"#)
                .expect("decode array")
                .expect("complete array");
        assert_eq!(array.result, serde_json::json!([1, 2]));
        let string = decode_eval_wire(
            r#"{"status":"complete","value_type":"string","result_json":"\"hello\""}"#,
        )
        .expect("decode string")
        .expect("complete string");
        assert_eq!(string.result, serde_json::json!("hello"));
    }

    #[test]
    fn rejects_malformed_result_instead_of_returning_corrupt_data() {
        assert!(
            decode_eval_wire(
                r#"{"status":"complete","value_type":"array","result_json":"1.02.0"}"#
            )
            .is_err()
        );
    }
}
