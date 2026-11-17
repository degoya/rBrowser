// The page script of the form autofill (RB-070-11): the component runs it in a view through
// `evaluate`, in the plugin's isolated world, as `(<this function>)(<request as JSON>)`
// (src/fill.rs). `collect` reads the fields of the document or of one element (`selector`) with
// their hints; `apply` sets the planned values (src/plan.rs) field by field. Values go through the
// prototype's setter, then `input` and `change` bubble, as the sync agent's form apply does
// (agent/src/forms/apply.ts) - frameworks see them - and checkboxes and radios are clicked. The
// events are not trusted, so the sync agent relays none of them: every view is filled by its own
// run. `collect` also names the document's forms - each with a selector that finds it alone -
// and each field its form, so a person can choose one. Nothing is submitted: a submit while the values are set is cancelled and counted, buttons
// are never clicked and file inputs never touched. `capture` (RB-090-70) reads the current values
// of one form - the selector's element, the focused field's form, the only form, else the whole
// document - for a preset; a password field, a field that names a secret (the rule of
// src/secret.rs, `secretOf`) and a file input give no value, so it never leaves the page.
// `type=hidden` inputs are fields only with `hiddenInputs` (the setting captureHiddenInputs).
// @ts-check
/** @param {{ op: string, selector?: string | null, skipHidden?: boolean, hiddenInputs?: boolean, actions?: any[] }} request */
(request) => {
  'use strict';
  const MAX_FIELDS = 300;
  const MAX_OPTIONS = 200;
  const MAX_TEXT = 200;
  const MAX_FORMS = 50;
  const MAX_SELECTOR = 1024;
  /** Inputs that are no fields: they are not reported at all. */
  const NOT_FIELDS = ['hidden', 'submit', 'button', 'reset', 'image'];
  /** The longest value a preset takes (src/settings.rs MAX_VALUE). */
  const MAX_VALUE = 1000;
  /** The secret rule of src/secret.rs: `autocomplete` tokens, whole words and parts of words. */
  const PASSWORD_TOKENS = ['current-password', 'new-password'];
  const SECRET_TOKENS = ['one-time-code', 'cc-number', 'cc-csc', 'cc-exp', 'cc-exp-month', 'cc-exp-year'];
  const SECRET_WORDS = ['pin', 'pincode', 'otp', 'cvv', 'cvc', 'csc', 'pwd', 'pass', 'passcode', 'ssn'];
  const SECRET_PARTS = ['password', 'passwort', 'passwd', 'kennwort', 'secret', 'token', 'csrf', 'xsrf', 'apikey',
    'onetime', 'securitycode', 'verificationcode', 'cardnumber', 'creditcard', 'ccnumber'];
  const FLAGS = ['valueMissing', 'typeMismatch', 'patternMismatch', 'tooLong', 'tooShort',
    'rangeUnderflow', 'rangeOverflow', 'stepMismatch', 'badInput', 'customError'];

  /** @param {unknown} text */
  const cut = (text) => String(text ?? '').replace(/\s+/g, ' ').trim().slice(0, MAX_TEXT);

  /** @param {Element} element @returns {string} */
  const typeOf = (element) =>
    element instanceof HTMLInputElement ? (element.type || 'text').toLowerCase() : element.localName;

  /** @param {Element} element */
  const sig = (element) =>
    [element.localName, typeOf(element), element.getAttribute('name') ?? '', element.id].join('|');

  /** @param {Element} element */
  function shown(element) {
    if (typeof element.checkVisibility === 'function') return element.checkVisibility();
    return element.getClientRects().length > 0;
  }

  /** The text of the elements `ids` names (aria-labelledby). @param {string} ids */
  function byIds(ids) {
    return ids.split(/\s+/).map((id) => (id ? document.getElementById(id)?.textContent ?? '' : '')).join(' ');
  }

  /** @param {HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement} element */
  function labelOf(element) {
    const own = Array.from(element.labels ?? []).map((label) => label.textContent ?? '').join(' ');
    const named = element.getAttribute('aria-labelledby');
    return cut(own || element.getAttribute('aria-label') || (named ? byIds(named) : '') || element.title);
  }

  /** The value attribute, if the element has it. @param {Element} element @param {string} name */
  const attr = (element, name) => cut(element.getAttribute(name));

  /** `text` as a CSS identifier or string body. @param {string} text */
  const escape = (text) =>
    typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
      ? CSS.escape(text)
      : text.replace(/[^a-zA-Z0-9_-]/g, (char) => `\\${char.codePointAt(0)?.toString(16)} `);

  /** Whether `selector` finds `element` and nothing else. @param {string} selector @param {Element} element */
  function alone(selector, element) {
    try {
      const found = document.querySelectorAll(selector);
      return found.length === 1 && found[0] === element;
    } catch {
      return false;
    }
  }

  /** A selector that finds `form` alone: its id, its name, else its path from an element with an id. @param {HTMLFormElement} form */
  function selectorOf(form) {
    const id = form.getAttribute('id');
    if (id && alone(`#${escape(id)}`, form)) return `#${escape(id)}`;
    const name = form.getAttribute('name');
    if (name && alone(`form[name="${escape(name)}"]`, form)) return `form[name="${escape(name)}"]`;
    const steps = [];
    for (let node = /** @type {Element | null} */ (form); node && node.parentElement; node = node.parentElement) {
      const own = node === form ? '' : node.getAttribute('id');
      if (own && alone(`#${escape(own)}`, node)) {
        steps.unshift(`#${escape(own)}`);
        break;
      }
      steps.unshift(`${node.localName}:nth-child(${Array.from(node.parentElement.children).indexOf(node) + 1})`);
    }
    return steps.join(' > ').slice(0, MAX_SELECTOR);
  }

  /** A form's selector and name. @param {HTMLFormElement} form */
  function formInfo(form) {
    const named = form.getAttribute('aria-labelledby');
    const name = form.getAttribute('aria-label') || (named ? byIds(named) : '') || form.getAttribute('name') || form.title || form.id;
    return { selector: selectorOf(form), name: cut(name) };
  }

  /** The document's forms (the first MAX_FORMS) with their selectors and names. */
  const formsOf = () => Array.from(document.forms).slice(0, MAX_FORMS).map(formInfo);

  /** A control's form by its position among the document's forms, else null. @param {Element} element */
  function formIndex(element) {
    const form = /** @type {HTMLInputElement} */ (element).form;
    const at = form ? Array.from(document.forms).indexOf(form) : -1;
    return at >= 0 && at < MAX_FORMS ? at : null;
  }

  /** The root the request names, or a reason it has none. */
  function root() {
    if (!request.selector) return { root: document };
    try {
      const found = document.querySelector(request.selector);
      return found ? { root: found } : { error: 'not-found' };
    } catch {
      return { error: 'selector' };
    }
  }

  /** Every control under `scope`, in document order. @param {ParentNode & Node} scope */
  function controls(scope) {
    const left = request.hiddenInputs === true ? NOT_FIELDS.filter((type) => type !== 'hidden') : NOT_FIELDS;
    const list = Array.from(scope.querySelectorAll('input, select, textarea'));
    if (scope instanceof Element && scope.matches('input, select, textarea')) list.unshift(scope);
    return /** @type {(HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement)[]} */ (
      list.filter((element) => !(element instanceof HTMLInputElement && left.includes(typeOf(element))))
    );
  }

  /** The radios of `radio`'s group. @param {HTMLInputElement} radio @param {Element[]} all */
  const group = (radio, all) =>
    /** @type {HTMLInputElement[]} */ (all.filter((other) => other instanceof HTMLInputElement &&
      other.type === 'radio' && other.name === radio.name && other.form === radio.form));

  /**
   * The fields: one per control, one per radio group (its first radio), with their hints.
   * @param {(HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement)[]} all
   */
  function fields(all) {
    /** @type {any[]} */
    const out = [];
    const seen = new Set();
    /** @type {Map<string, number>} */
    const keys = new Map();
    for (const [index, element] of all.entries()) {
      if (out.length >= MAX_FIELDS) return { out, truncated: true };
      const type = typeOf(element);
      const radios = element instanceof HTMLInputElement && type === 'radio' && element.name ? group(element, all) : null;
      if (radios) {
        if (seen.has(radios[0])) continue;
        seen.add(radios[0]);
      }
      const members = radios ?? [element];
      const name = element.getAttribute('name') ?? '';
      let key = `${element.localName}:${type}:${name || element.id || index}`;
      const repeat = keys.get(key) ?? 0;
      keys.set(key, repeat + 1);
      if (repeat > 0) key += `:${repeat}`;
      const legend = radios ? element.closest('fieldset')?.querySelector('legend')?.textContent : '';
      const input = element instanceof HTMLInputElement ? element : null;
      const text = element instanceof HTMLSelectElement ? null : /** @type {HTMLInputElement | HTMLTextAreaElement} */ (element);
      const options = element instanceof HTMLSelectElement
        ? Array.from(element.options).map((option) => ({ value: cut(option.value), label: cut(option.text), disabled: option.disabled }))
        : (radios ?? []).map((radio) => ({ value: cut(radio.value), label: labelOf(radio), disabled: radio.disabled }));
      out.push({
        index, key, sig: sig(element), tag: element.localName, type, name: cut(name), id: cut(element.id),
        autocomplete: attr(element, 'autocomplete'), label: cut(legend) || labelOf(element),
        placeholder: attr(element, 'placeholder'), pattern: String(element.getAttribute('pattern') ?? '').slice(0, 1024),
        min: attr(element, 'min'), max: attr(element, 'max'), step: attr(element, 'step'),
        minLength: text && text.minLength >= 0 ? text.minLength : null,
        maxLength: text && text.maxLength >= 0 ? text.maxLength : null,
        required: members.some((member) => member.required),
        multiple: element instanceof HTMLSelectElement ? element.multiple : Boolean(input?.multiple),
        options: options.slice(0, MAX_OPTIONS),
        skip: type === 'file' ? 'file'
          : members.every((member) => member.disabled) ? 'disabled'
          : text && text.readOnly ? 'readonly'
          : request.skipHidden !== false && type !== 'hidden' && !members.some(shown) ? 'hidden' : '',
        form: formIndex(element),
      });
    }
    return { out, truncated: false };
  }

  function locale() {
    try {
      return new Intl.DateTimeFormat().resolvedOptions().locale || navigator.language;
    } catch {
      return navigator.language || document.documentElement.lang || '';
    }
  }

  /** The setter of `property` on `element`'s prototype chain, past an own property. @param {Element} element @param {string} property */
  function setter(element, property) {
    for (let proto = Object.getPrototypeOf(element); proto; proto = Object.getPrototypeOf(proto)) {
      const set = Object.getOwnPropertyDescriptor(proto, property)?.set;
      if (set) return (/** @type {unknown} */ value) => set.call(element, value);
    }
    return (/** @type {unknown} */ value) => Reflect.set(element, property, value);
  }

  /** `input` (composed, like the engine's) and `change`, both bubbling. @param {Element} element */
  function fire(element) {
    const EventClass = element.ownerDocument.defaultView?.Event ?? Event;
    element.dispatchEvent(new EventClass('input', { bubbles: true, composed: true }));
    element.dispatchEvent(new EventClass('change', { bubbles: true }));
  }

  /** Sets the first candidate that meets the goal. @param {HTMLInputElement | HTMLTextAreaElement} element @param {any} action */
  function setText(element, action) {
    const candidates = Array.isArray(action.candidates) ? action.candidates.map(String) : [];
    const set = setter(element, 'value');
    let chosen = candidates[0] ?? '';
    for (const candidate of candidates) {
      set(candidate);
      const valid = element.validity.valid;
      if (action.goal === 'any' || (action.goal === 'valid' && valid) || (action.goal === 'invalid' && !valid)) {
        chosen = candidate;
        break;
      }
      if (action.goal === 'invalid') chosen = candidate;
    }
    set(chosen);
    fire(element);
  }

  /** @param {HTMLInputElement} input @param {boolean} checked */
  function setChecked(input, checked) {
    if (input.checked !== checked) input.click();
    if (input.checked !== checked) {
      // A radio cannot be clicked off; a page may cancel the click: set it as a script would.
      setter(input, 'checked')(checked);
      fire(input);
    }
  }

  /** @param {HTMLSelectElement} select @param {any} action */
  function setSelect(select, action) {
    const values = Array.isArray(action.values) ? action.values.map(String) : [];
    const options = Array.from(select.options);
    if (values.length === 0) {
      const blank = options.findIndex((option) => option.value === '');
      setter(select, 'selectedIndex')(blank);
    } else if (select.multiple) {
      for (const option of options) setter(option, 'selected')(values.includes(option.value));
    } else {
      setter(select, 'value')(values[0]);
    }
    fire(select);
  }

  /** What a field holds now. @param {Element} element @param {HTMLInputElement[] | null} radios */
  function holds(element, radios) {
    if (radios) return cut(radios.find((radio) => radio.checked)?.value ?? '');
    if (element instanceof HTMLInputElement && element.type === 'checkbox') return String(element.checked);
    if (element instanceof HTMLSelectElement) return cut(Array.from(element.selectedOptions).map((o) => o.value).join(', '));
    return String(/** @type {HTMLInputElement} */ (element).value ?? '').slice(0, MAX_TEXT);
  }

  /** @param {(HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement)[]} all */
  function apply(all) {
    let blocked = 0;
    const view = document.defaultView ?? window;
    /** @param {Event} event */
    const stop = (event) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      blocked += 1;
    };
    view.addEventListener('submit', stop, true);
    /** @type {any[]} */
    const results = [];
    /** @type {Set<HTMLFormElement>} */
    const forms = new Set();
    const touched = [];
    try {
      for (const action of request.actions ?? []) {
        const element = all[action.index];
        if (!element) {
          results.push({ index: action.index, status: 'missing' });
          continue;
        }
        if (sig(element) !== action.sig) {
          results.push({ index: action.index, status: 'changed' });
          continue;
        }
        const radios = element instanceof HTMLInputElement && element.type === 'radio' ? group(element, all) : null;
        element.focus({ preventScroll: true });
        if (action.op === 'radio' && radios) {
          const target = radios.find((radio) => radio.value === action.value);
          if (target) setChecked(target, true);
          else for (const radio of radios) if (radio.checked) setChecked(radio, false);
        } else if (action.op === 'check' && element instanceof HTMLInputElement) {
          setChecked(element, Boolean(action.checked));
        } else if (action.op === 'select' && element instanceof HTMLSelectElement) {
          setSelect(element, action);
        } else if (action.op === 'text' && !(element instanceof HTMLSelectElement)) {
          setText(element, action);
        }
        element.blur();
        if (element.form) forms.add(element.form);
        touched.push(element);
        const errors = FLAGS.filter((flag) => element.validity[/** @type {keyof ValidityState} */ (flag)] === true);
        results.push({ index: action.index, status: 'done', value: holds(element, radios), valid: element.validity.valid, errors });
      }
      const formValid = forms.size > 0
        ? Array.from(forms).every((form) => form.checkValidity())
        : touched.length > 0 ? touched.every((element) => element.validity.valid) : null;
      return { fields: results, formValid, blockedSubmits: blocked };
    } finally {
      view.removeEventListener('submit', stop, true);
    }
  }

  /** The words of `text` as src/secret.rs splits them. @param {string} text */
  const words = (text) => text.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);

  /** Whether `text` names a secret. @param {string} text */
  function secretText(text) {
    const list = words(text);
    const compact = list.join('');
    return list.some((word) => SECRET_WORDS.includes(word)) || SECRET_PARTS.some((part) => compact.includes(part));
  }

  /** `password`, `secret` or '' (src/secret.rs `reason`). @param {HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement} element */
  function secretOf(element) {
    const tokens = (element.getAttribute('autocomplete') ?? '').toLowerCase().split(/\s+/).filter(Boolean);
    if (typeOf(element) === 'password' || tokens.some((token) => PASSWORD_TOKENS.includes(token))) return 'password';
    const named = [element.getAttribute('name') ?? '', element.id, labelOf(element), element.getAttribute('placeholder') ?? ''];
    return tokens.some((token) => SECRET_TOKENS.includes(token)) || named.some(secretText) ? 'secret' : '';
  }

  /**
   * What a preset takes from a field, or why it takes nothing; a secret's value stays here.
   * @param {HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement} element @param {HTMLInputElement[] | null} radios
   */
  function captured(element, radios) {
    if (typeOf(element) === 'file') return { excluded: 'file' };
    const secret = secretOf(element);
    if (secret) return { excluded: secret };
    if (radios) return { value: radios.find((radio) => radio.checked)?.value ?? '' };
    if (element instanceof HTMLInputElement && element.type === 'checkbox') return { value: String(element.checked) };
    if (element instanceof HTMLSelectElement) {
      const chosen = Array.from(element.selectedOptions);
      return chosen.length > 1 ? { excluded: 'multiple' } : { value: chosen[0]?.value ?? '' };
    }
    const value = String(element.value ?? '');
    return Array.from(value).length > MAX_VALUE ? { excluded: 'long' } : { value };
  }

  /** The root a capture reads, and how it was found. @returns {{ root?: ParentNode & Node, error?: string, scope: string }} */
  function captureRoot() {
    if (request.selector) return { ...root(), scope: 'selector' };
    const active = /** @type {any} */ (document.activeElement);
    const form = active?.form instanceof HTMLFormElement ? active.form : active?.closest?.('form') ?? null;
    if (form) return { root: form, scope: 'focused' };
    if (document.forms.length === 1) return { root: document.forms[0], scope: 'form' };
    return { root: document, scope: 'document' };
  }

  /** The current values of the capture's root, field by field. */
  function capture() {
    const found = captureRoot();
    const page = { url: location.href, title: cut(document.title), locale: locale(), scope: found.scope };
    if (!found.root) return { ...page, error: found.error, form: null, fields: [] };
    const all = controls(found.root);
    const { out, truncated } = fields(all);
    for (const field of out) {
      const element = all[field.index];
      if (!element) continue;
      const radios = element instanceof HTMLInputElement && element.type === 'radio' && element.name ? group(element, all) : null;
      Object.assign(field, captured(element, radios));
    }
    const form = found.root instanceof HTMLFormElement ? formInfo(found.root) : null;
    return { ...page, form, fields: out, truncated };
  }

  if (request.op === 'capture') return capture();
  const found = root();
  if (!found.root) return { error: found.error, locale: locale(), url: location.href, forms: formsOf(), fields: [] };
  const all = controls(found.root);
  if (request.op === 'apply') return apply(all);
  const { out, truncated } = fields(all);
  return { locale: locale(), url: location.href, forms: formsOf(), fields: out, truncated };
}
