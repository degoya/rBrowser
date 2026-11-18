// The sync agent's selector for the form autofill's page script (RB-090-78): a copy of
// `centreTarget` of agent/src/selector.ts with agent/src/role.ts - `data-testid`, ARIA role and
// accessible name, a stable unique id, tag and text, the CSS path - so a field the autofill sets
// is named in a recording as the field a person types into. src/fill.rs runs it as
// `(<this function>)()` and hands the `targetOf` it answers to src/page.js;
// web/src/record.test.ts holds it equal to the agent's.
// @ts-check
/** @returns {(element: Element) => unknown} */
() => {
  'use strict';
  /** `text` as a CSS identifier or string body. @param {string} text */
  const escape = (text) =>
    typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
      ? CSS.escape(text)
      : text.replace(/[^a-zA-Z0-9_-]/g, (char) => `\\${char.codePointAt(0)?.toString(16)} `);

  const GENERATED_PREFIX = /^(css|sc|jsx|jss|emotion|styled|svelte|astro)-/i;
  const CSS_MODULE = /__[A-Za-z0-9_-]{5,}$/;
  const MIXED_CASE = /^[a-zA-Z][a-z]*[A-Z][a-z]*[A-Z][a-zA-Z]*$/;
  const STATE_CLASS =
    /^((is|has)-.*|active|current|selected|open|opened|closed|expanded|collapsed|checked|disabled|focus|focused|hover|hovered|visible|hidden|show|shown|in|fade)$/;
  const PLAIN = /^[A-Za-z_-][A-Za-z0-9_-]*$/;
  const MAX_NAME = 120;
  const MAX_SELECTOR_TEXT = 80;
  const CONTROLS = 'a[href],button,input,select,textarea';
  const NAME_FROM_CONTENT = ['button', 'link', 'heading', 'checkbox', 'radio', 'switch', 'tab', 'menuitem',
    'menuitemcheckbox', 'menuitemradio', 'option', 'treeitem', 'cell', 'columnheader', 'rowheader'];
  /** @type {Record<string, string>} */
  const ROLE_TAGS = { button: 'button,input,summary', link: 'a,area', checkbox: 'input', radio: 'input',
    slider: 'input', spinbutton: 'input', searchbox: 'input', textbox: 'input,textarea', combobox: 'select',
    listbox: 'select', option: 'option', heading: 'h1,h2,h3,h4,h5,h6', img: 'img', navigation: 'nav' };
  /** @type {Record<string, string>} */
  const INPUT_ROLES = { button: 'button', submit: 'button', reset: 'button', image: 'button', checkbox: 'checkbox',
    radio: 'radio', range: 'slider', number: 'spinbutton', search: 'searchbox', email: 'textbox', tel: 'textbox',
    text: 'textbox', url: 'textbox' };

  /** @param {string | null | undefined} text */
  const normalize = (text) => (text ?? '').replace(/\s+/g, ' ').trim();

  /** @param {string} name */
  const isHash = (name) => /^[a-z0-9]{5,}$/i.test(name) && /[a-z]\d+[a-z]/i.test(name);

  /** @param {string} name */
  const isGeneratedClass = (name) => GENERATED_PREFIX.test(name) || CSS_MODULE.test(name) || isHash(name) ||
    (name.length >= 5 && name.length <= 8 && MIXED_CASE.test(name));

  /** @param {string} id */
  const isGeneratedId = (id) => id.includes(':') || /^\d/.test(id) || /\d{4,}/.test(id) ||
    /[0-9a-f]{8}-[0-9a-f]{4}/i.test(id) || isHash(id);

  /** @param {Element} element @returns {Document | ShadowRoot} */
  function rootOf(element) {
    const root = element.getRootNode();
    return root instanceof ShadowRoot ? root : element.ownerDocument;
  }

  /** @param {Element} node */
  function pathStep(node) {
    const tag = escape(node.localName);
    const parent = node.parentElement ?? /** @type {ParentNode | null} */ (node.parentNode);
    if (!parent) return tag;
    const same = Array.from(parent.children).filter((child) => child.localName === node.localName);
    if (same.length === 1) return tag;
    const stable = Array.from(node.classList).filter((name) => PLAIN.test(name) && !isGeneratedClass(name) && !STATE_CLASS.test(name));
    for (const name of stable) {
      if (same.filter((child) => child.classList.contains(name)).length === 1) return `${tag}.${escape(name)}`;
    }
    return `${tag}:nth-of-type(${same.indexOf(node) + 1})`;
  }

  /** The CSS path of `element` in its root: up to the nearest stable unique id. @param {Element} element */
  function cssPath(element) {
    const root = rootOf(element);
    /** @type {string[]} */
    const parts = [];
    for (let node = /** @type {Element | null} */ (element); node; node = node.parentElement) {
      if (node.id && !isGeneratedId(node.id)) {
        const byId = `#${escape(node.id)}`;
        if (root.querySelectorAll(byId).length === 1) {
          parts.unshift(byId);
          break;
        }
      }
      parts.unshift(pathStep(node));
    }
    return parts.join(' > ');
  }

  /** @param {Element} element @returns {string | null} */
  function roleOf(element) {
    const explicit = normalize(element.getAttribute('role')).split(' ')[0];
    if (explicit) return explicit;
    switch (element.localName) {
      case 'button': case 'summary': return 'button';
      case 'a': case 'area': return element.hasAttribute('href') ? 'link' : null;
      case 'input': return INPUT_ROLES[/** @type {HTMLInputElement} */ (element).type] ?? null;
      case 'textarea': return 'textbox';
      case 'select': {
        const select = /** @type {HTMLSelectElement} */ (element);
        return select.multiple || select.size > 1 ? 'listbox' : 'combobox';
      }
      case 'option': return 'option';
      case 'h1': case 'h2': case 'h3': case 'h4': case 'h5': case 'h6': return 'heading';
      case 'img': return element.getAttribute('alt') ? 'img' : null;
      case 'nav': return 'navigation';
      default: return null;
    }
  }

  /** @param {Element} element @param {string} ids */
  function textOfIds(element, ids) {
    const root = /** @type {Document | ShadowRoot} */ (element.getRootNode());
    return normalize(ids.split(/\s+/).map((id) => (id ? root.getElementById?.(id)?.textContent : '') ?? '').join(' '));
  }

  /** The text of a field's labels, without the fields in them. @param {Element} element */
  function labelText(element) {
    const labels = /** @type {HTMLInputElement} */ (element).labels;
    if (!labels || labels.length === 0) return '';
    return normalize(Array.from(labels).map((label) => {
      const copy = /** @type {Element} */ (label.cloneNode(true));
      copy.querySelectorAll('input, select, textarea').forEach((field) => field.remove());
      return copy.textContent;
    }).join(' '));
  }

  /** The accessible name, '' when none. @param {Element} element @param {string | null} role */
  function nameOf(element, role) {
    const labelledBy = element.getAttribute('aria-labelledby');
    if (labelledBy) {
      const name = textOfIds(element, labelledBy);
      if (name) return name;
    }
    const label = normalize(element.getAttribute('aria-label'));
    if (label) return label;
    const tag = element.localName;
    const input = /** @type {HTMLInputElement} */ (element);
    if (tag === 'input' || tag === 'select' || tag === 'textarea') {
      const fromLabels = labelText(element);
      if (fromLabels) return fromLabels;
      if (tag === 'input' && ['button', 'submit', 'reset'].includes(input.type)) return normalize(input.value);
    }
    if (tag === 'img' || (tag === 'input' && input.type === 'image')) {
      const alt = normalize(element.getAttribute('alt'));
      if (alt) return alt;
    }
    if (role && NAME_FROM_CONTENT.includes(role)) {
      const text = normalize(element.textContent);
      if (text) return text;
    }
    return normalize(element.getAttribute('title') ?? element.getAttribute('placeholder'));
  }

  /** Every element `selector` matches in `root`, in document order. @param {any} selector @param {Document | ShadowRoot} root @returns {Element[]} */
  function matchAll(selector, root) {
    try {
      if (selector.kind === 'testid') return Array.from(root.querySelectorAll(`[data-testid="${escape(selector.testid)}"]`));
      if (selector.kind === 'role') {
        const tags = ROLE_TAGS[selector.role];
        const query = `[role="${escape(selector.role)}"]${tags ? `,${tags}` : ''}`;
        return Array.from(root.querySelectorAll(query)).filter((element) => {
          const role = roleOf(element);
          return role === selector.role && nameOf(element, role) === selector.name;
        });
      }
      if (selector.kind === 'text') {
        return Array.from(root.querySelectorAll(escape(selector.tag))).filter((element) => normalize(element.textContent) === selector.text);
      }
      return Array.from(root.querySelectorAll(selector.path));
    } catch {
      return [];
    }
  }

  /** @param {any} selector @param {number} nth */
  const withNth = (selector, nth) => (nth > 0 ? { ...selector, nth } : selector);

  /** The strongest selector that finds `element`. @param {Element} element @returns {any} */
  function makeSelector(element) {
    const root = rootOf(element);
    const path = cssPath(element);
    /** @type {any[]} */
    const candidates = [];
    const testid = element.getAttribute('data-testid');
    if (testid) candidates.push({ kind: 'testid', testid, path });
    const role = roleOf(element);
    const name = role ? nameOf(element, role) : '';
    if (role && name && name.length <= MAX_NAME) candidates.push({ kind: 'role', role, name, path });
    for (const candidate of candidates) {
      const nth = matchAll(candidate, root).indexOf(element);
      if (nth >= 0) return withNth(candidate, nth);
    }
    if (path.startsWith('#') && !path.includes(' ')) return { kind: 'css', path };
    const text = normalize(element.textContent);
    const tag = element.localName;
    if (text && text.length <= MAX_SELECTOR_TEXT && tag !== 'html' && tag !== 'body' && !element.querySelector(CONTROLS)) {
      const candidate = { kind: 'text', tag, text, path };
      const nth = matchAll(candidate, root).indexOf(element);
      if (nth >= 0) return withNth(candidate, nth);
    }
    return { kind: 'css', path };
  }

  /** The agent's target of an element without a pointer: its selector, the shadow hosts above it, its centre. @param {Element} element */
  function targetOf(element) {
    /** @type {Element[]} */
    const hosts = [];
    for (let root = element.getRootNode(); root instanceof ShadowRoot; ) {
      hosts.unshift(root.host);
      root = root.host.getRootNode();
    }
    /** @type {any} */
    const target = { element: makeSelector(element), offset: { x: 0.5, y: 0.5 } };
    if (hosts.length > 0) target.hosts = hosts.map(makeSelector);
    return target;
  }

  return targetOf;
}
