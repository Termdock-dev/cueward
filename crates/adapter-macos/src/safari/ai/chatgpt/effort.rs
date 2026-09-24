use crate::MacosError;

use super::super::super::eval::exec;
use super::super::super::interaction::RUNTIME;

fn build_effort_js(choice: &str) -> Result<String, MacosError> {
    if choice != "pro" && choice.parse::<u32>().is_err() {
        return Err(MacosError::Other(
            "ChatGPT --effort must be a slider number or 'pro'".to_string(),
        ));
    }
    let choice = serde_json::to_string(choice)
        .map_err(|error| MacosError::Other(format!("invalid effort: {error}")))?;
    Ok(format!(
        r#"
        {RUNTIME}
        const choice = {choice};
        const pill = document.querySelector('button.__composer-pill');
        if (!pill) throw new Error('ChatGPT effort control not found');
        if (pill.getAttribute('aria-expanded') !== 'true') cuewardClick(pill);
        let slider;
        for (let attempt = 0; attempt < 10; attempt++) {{
          slider = document.querySelector('[role="menu"] [role="slider"]');
          if (slider) break;
          await new Promise(resolve => setTimeout(resolve, 100));
        }}
        if (!slider) throw new Error('ChatGPT effort slider not found');
        const min = Number(slider.getAttribute('aria-valuemin'));
        const max = Number(slider.getAttribute('aria-valuemax'));
        let current = Number(slider.getAttribute('aria-valuenow'));
        if (![min, max, current].every(Number.isInteger) || min > max || current < min || current > max)
          throw new Error('ChatGPT effort slider has invalid ARIA values');
        const target = choice === 'pro' ? max : Number(choice);
        if (!Number.isInteger(target) || target < min || target > max)
          throw new Error(`ChatGPT effort ${{choice}} is outside slider range ${{min}}-${{max}}`);
        slider.focus();
        for (let attempt = 0; current !== target && attempt <= max - min; attempt++) {{
          const key = current < target ? 'ArrowRight' : 'ArrowLeft';
          const keyCode = key === 'ArrowRight' ? 39 : 37;
          const event = type => new KeyboardEvent(type, {{
            key, code: key, keyCode, which: keyCode,
            bubbles: true, cancelable: true, composed: true
          }});
          slider.dispatchEvent(event('keydown'));
          slider.dispatchEvent(event('keyup'));
          await new Promise(resolve => setTimeout(resolve, 100));
          const next = Number(slider.getAttribute('aria-valuenow'));
          if (!Number.isInteger(next) || next === current)
            throw new Error('ChatGPT effort slider did not respond to arrow key');
          current = next;
        }}
        if (current !== target) throw new Error('ChatGPT effort slider did not reach requested level');
        return {{level: current, min, max}};
        "#
    ))
}

pub fn set_chatgpt_effort(choice: &str, profile_filter: Option<&str>) -> Result<(), MacosError> {
    let code = build_effort_js(choice)?;
    let value = exec(&code, profile_filter, None, 10, true)?.result;
    if value
        .get("level")
        .and_then(serde_json::Value::as_i64)
        .is_none()
    {
        return Err(MacosError::Other(
            "ChatGPT effort result did not contain a level".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::build_effort_js;

    #[test]
    fn validates_choice_before_browser_interaction() {
        assert!(build_effort_js("pro").is_ok());
        assert!(build_effort_js("4").is_ok());
        assert!(build_effort_js("invalid").is_err());
        assert!(build_effort_js("-1").is_err());
    }

    #[test]
    fn moves_slider_to_dynamic_maximum() {
        let code = build_effort_js("pro").expect("effort script");
        let script = format!(
            r#"
            globalThis.window = globalThis;
            globalThis.PointerEvent = class {{ constructor(type, options) {{ this.type = type; Object.assign(this, options); }} }};
            globalThis.MouseEvent = globalThis.PointerEvent;
            globalThis.KeyboardEvent = globalThis.PointerEvent;
            let expanded = false;
            let value = 3;
            const pill = {{
              matches: () => false,
              getAttribute: name => name === 'aria-expanded' ? String(expanded) : null,
              dispatchEvent: event => {{ if (event.type === 'pointerdown') expanded = true; }},
              click: () => {{}}
            }};
            const slider = {{
              getAttribute: name => ({{'aria-valuemin': '0', 'aria-valuemax': '4', 'aria-valuenow': String(value)}})[name],
              focus: () => {{}},
              dispatchEvent: event => {{ if (event.type === 'keydown') value += event.key === 'ArrowRight' ? 1 : -1; }}
            }};
            globalThis.document = {{querySelector: selector =>
              selector === 'button.__composer-pill' ? pill :
              selector === '[role="menu"] [role="slider"]' && expanded ? slider : null}};
            (async () => {{ {code} }})().then(result => process.stdout.write(JSON.stringify(result)));
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
            serde_json::from_slice(&output.stdout).expect("effort JSON");
        assert_eq!(result["level"], 4);
        assert_eq!(result["max"], 4);
    }
}
