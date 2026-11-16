// The page script of the form autofill (RB-070-11): the component runs it in a view through
// `evaluate`, in the plugin's isolated world, as `(<this function>)(<request as JSON>)`
// (src/fill.rs). `collect` reads the fields of the document or of one element (`selector`) with
// their hints; `apply` sets the planned values (src/plan.rs) field by field. Values go through the
// prototype's setter, then `input` and `change` bubble, as the sync agent's form apply does
// (agent/src/forms/apply.ts) - frameworks see them - and checkboxes and radios are clicked. The
// events are not trusted, so the sync agent relays none of them: every view is filled by its own
// run. `collect` also names the document's forms - each with a selector that finds it alone -
// and each field its form, so a person can choose one. Nothing is submitted: a submit while the values are set is cancelled and counted, buttons
// are never clicked and file inputs never touched.
// @ts-check
/** @param {{ op: string, selector?: string | null, skipHidden?: boolean, actions?: any[] }} request */
(request) => {
  'use strict';
  const MAX_FIELDS = 300;
  const MAX_OPTIONS = 200;
  const MAX_TEXT = 200;
  const MAX_FORMS = 50;
  const MAX_SELECTOR = 1024;
  /** Inputs that are no fields: they are not reported at all. */
  const NOT_FIELDS = ['hidden', 'submit', 'button', 'reset', 'image'];
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

  /** The document's forms (the first MAX_FORMS) with their selectors and names. */
  function formsOf() {
    return Array.from(document.forms).slice(0, MAX_FORMS).map((form) => {
      const named = form.getAttribute('aria-labelledby');
      const name = form.getAttribute('aria-label') || (named ? byIds(named) : '') || form.getAttribute('name') || form.title || form.id;
      return { selector: selectorOf(form), name: cut(name) };
    });
  }

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
    const list = Array.from(scope.querySelectorAll('input, select, textarea'));
    if (scope instanceof Element && scope.matches('input, select, textarea')) list.unshift(scope);
    return /** @type {(HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement)[]} */ (
      list.filter((element) => !(element instanceof HTMLInputElement && NOT_FIELDS.includes(typeOf(element))))
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
          : request.skipHidden !== false && !members.some(shown) ? 'hidden' : '',
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

  const found = root();
  if (!found.root) return { error: found.error, locale: locale(), url: location.href, forms: formsOf(), fields: [] };
  const all = controls(found.root);
  if (request.op === 'apply') return apply(all);
  const { out, truncated } = fields(all);
  return { locale: locale(), url: location.href, forms: formsOf(), fields: out, truncated };
}
