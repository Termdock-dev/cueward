use serde_json::json;

pub(super) const RUNTIME: &str = r#"
  const cuewardVisitRoots = (root, visit) => {
    visit(root);
    for (const node of root.querySelectorAll?.('*') || []) {
      if (node.shadowRoot) cuewardVisitRoots(node.shadowRoot, visit);
      if (node.tagName === 'IFRAME') {
        try {
          if (node.contentDocument) cuewardVisitRoots(node.contentDocument, visit);
        } catch (_) { /* cross-origin frames are inaccessible */ }
      }
    }
  };
  const cuewardQuery = (selector) => {
    let found = null;
    cuewardVisitRoots(document, root => {
      if (!found) found = root.querySelector?.(selector) || null;
    });
    return found;
  };
  const cuewardFindText = (text) => {
    const exact = [];
    const partial = [];
    cuewardVisitRoots(document, root => {
      for (const node of root.querySelectorAll?.('*') || []) {
        const value = (node.innerText || '').trim();
        if (!value || node.getClientRects?.().length === 0) continue;
        if (value === text) exact.push(node);
        else if (value.includes(text)) partial.push(node);
      }
    });
    const candidates = exact.length ? exact : partial;
    return candidates.sort((a, b) => a.innerText.length - b.innerText.length)[0] || null;
  };
  const cuewardResolve = (target) => {
    const keys = ['ref', 'selector', 'text'].filter(key => target[key] != null);
    if (keys.length !== 1) throw new Error('target requires one ref, selector, or text');
    let el;
    if (target.ref != null) {
      const state = window.__cuewardRefState;
      el = state?.nodes?.get(String(target.ref));
      if (!el || !el.isConnected) throw new Error('stale or unknown element ref');
    } else if (target.selector != null) {
      el = cuewardQuery(target.selector);
    } else {
      el = cuewardFindText(target.text);
    }
    if (!el) throw new Error('target element not found');
    return el;
  };
  const cuewardEnabled = (el) => {
    if (el.matches?.(':disabled') || el.getAttribute?.('aria-disabled') === 'true') {
      throw new Error('element is disabled');
    }
  };
  const cuewardClick = (el) => {
    cuewardEnabled(el);
    const realm = el.ownerDocument?.defaultView || globalThis;
    const Pointer = realm.PointerEvent || PointerEvent;
    const Mouse = realm.MouseEvent || MouseEvent;
    const pointer = {bubbles: true, cancelable: true, composed: true,
      pointerType: 'mouse', button: 0, buttons: 1, isPrimary: true};
    const mouse = {bubbles: true, cancelable: true, composed: true,
      button: 0, buttons: 1};
    el.dispatchEvent(new Pointer('pointerover', pointer));
    el.dispatchEvent(new Pointer('pointerenter', {...pointer, bubbles: false}));
    el.dispatchEvent(new Mouse('mouseover', mouse));
    el.dispatchEvent(new Mouse('mouseenter', {...mouse, bubbles: false}));
    el.dispatchEvent(new Pointer('pointerdown', pointer));
    el.dispatchEvent(new Mouse('mousedown', mouse));
    el.dispatchEvent(new Pointer('pointerup', {...pointer, buttons: 0}));
    el.dispatchEvent(new Mouse('mouseup', {...mouse, buttons: 0}));
    if (typeof el.click === 'function') el.click();
    else el.dispatchEvent(new Mouse('click', mouse));
  };
  const cuewardFill = (el, text) => {
    cuewardEnabled(el);
    const doc = el.ownerDocument || document;
    const realm = doc.defaultView || globalThis;
    if (el instanceof realm.HTMLInputElement || el instanceof realm.HTMLTextAreaElement) {
      const proto = el instanceof realm.HTMLTextAreaElement
        ? realm.HTMLTextAreaElement.prototype : realm.HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
      if (!setter) throw new Error('native value setter unavailable');
      setter.call(el, text);
      el.dispatchEvent(new realm.Event('input', {bubbles: true, composed: true}));
      el.dispatchEvent(new realm.Event('change', {bubbles: true, composed: true}));
      if (el.value !== text) throw new Error('input value did not persist');
    } else if (el.isContentEditable) {
      el.focus();
      const selection = doc.getSelection?.() || realm.getSelection?.();
      if (!selection) throw new Error('contenteditable selection unavailable');
      const range = doc.createRange();
      range.selectNodeContents(el);
      selection.removeAllRanges();
      selection.addRange(range);
      if (!doc.execCommand('insertText', false, text)) {
        throw new Error('contenteditable insertText failed');
      }
    } else {
      throw new Error('fill requires input, textarea, or contenteditable');
    }
  };
  const cuewardKey = (el, key, options = {}) => {
    cuewardEnabled(el);
    const realm = el.ownerDocument?.defaultView || globalThis;
    el.focus?.();
    const init = {key, code: options.code || key, bubbles: true,
      cancelable: true, composed: true, ctrlKey: !!options.ctrl,
      altKey: !!options.alt, metaKey: !!options.meta, shiftKey: !!options.shift};
    el.dispatchEvent(new realm.KeyboardEvent('keydown', init));
    el.dispatchEvent(new realm.KeyboardEvent('keyup', init));
  };
  const cuewardSelect = (el, value) => {
    cuewardEnabled(el);
    const realm = el.ownerDocument?.defaultView || globalThis;
    if (!(el instanceof realm.HTMLSelectElement)) throw new Error('select requires a select element');
    el.value = value;
    if (el.value !== value) throw new Error('select option not found');
    el.dispatchEvent(new realm.Event('input', {bubbles: true, composed: true}));
    el.dispatchEvent(new realm.Event('change', {bubbles: true, composed: true}));
  };
  const cuewardCheck = (el, checked) => {
    cuewardEnabled(el);
    const realm = el.ownerDocument?.defaultView || globalThis;
    if (!(el instanceof realm.HTMLInputElement) || !['checkbox', 'radio'].includes(el.type)) {
      throw new Error('check requires a checkbox or radio input');
    }
    const setter = Object.getOwnPropertyDescriptor(realm.HTMLInputElement.prototype, 'checked')?.set;
    if (!setter) throw new Error('native checked setter unavailable');
    setter.call(el, checked);
    el.dispatchEvent(new realm.Event('input', {bubbles: true, composed: true}));
    el.dispatchEvent(new realm.Event('change', {bubbles: true, composed: true}));
    if (el.checked !== checked) throw new Error('checked state did not persist');
  };
  const cuewardScrollIntoView = (el) => {
    el.scrollIntoView({block: 'center', inline: 'nearest'});
  };
"#;

pub(super) fn selector_click_js(selector: &str) -> String {
    let target = json!({"selector": selector});
    format!(
        r#"(() => {{
          {RUNTIME}
          try {{ cuewardClick(cuewardResolve({target})); return 'true'; }}
          catch (error) {{ return String(error?.message || error); }}
        }})()"#
    )
}

pub(super) fn selector_fill_js(selector: &str, text: &str) -> String {
    let target = json!({"selector": selector});
    let value = json!(text);
    format!(
        r#"(() => {{
          {RUNTIME}
          try {{ cuewardFill(cuewardResolve({target}), {value}); return 'true'; }}
          catch (error) {{ return String(error?.message || error); }}
        }})()"#
    )
}
