// @ts-check
// The first-party plugin panels' syntax highlighter (RB-080-65) over the tokens of
// highlight-tokens.js. `render` builds text nodes and `<span class="hl-<kind>">` with
// `textContent` only: what it shows is page content, foreign, and never becomes markup. Above
// LIMIT_BYTES it shows the text in one colour with the panel's note; with `ui.syntaxHighlight`
// off (`configure`), always. `editor` colours a textarea over its own, transparent text. The
// colours are `--hl-<kind>` variables, light and dark, that a panel's own `:root` overrides.
//
// One source, this file. The classic-script panels that load it (console, network-panel,
// snippets, storage-panel) list it in their `web-common.txt`, and scripts/build-plugins.sh packs
// it into their panel/ (scripts/lib/plugin-stage.sh); their tests load it from here. No copies:
// plugins/web-common/src/highlight.test.ts holds the lists and pages to it. A classic script:
// `CodeHighlight` is a global.
globalThis.CodeHighlight = (() => {
  'use strict';
  const { LIMIT_BYTES, LANGS, tokenize, detect, tooLarge } = CodeTokens;
  const SETTING = 'ui.syntaxHighlight';
  const STYLE = [
    ':where(:root) { --hl-key: #953800; --hl-string: #0a3069; --hl-number: #0550ae; --hl-literal: #0550ae; --hl-keyword: #a40e26; --hl-comment: #57606a; --hl-tag: #116329; --hl-attr: #953800; --hl-fn: #6639ba; --hl-meta: #57606a; }',
    '@media (prefers-color-scheme: dark) { :where(:root) { --hl-key: #ffa657; --hl-string: #a5d6ff; --hl-number: #79c0ff; --hl-literal: #79c0ff; --hl-keyword: #ff7b72; --hl-comment: #a0a8b2; --hl-tag: #7ee787; --hl-attr: #ffa657; --hl-fn: #d2a8ff; --hl-meta: #a0a8b2; } }',
    '.hl-key { color: var(--hl-key); }',
    '.hl-string { color: var(--hl-string); }',
    '.hl-number { color: var(--hl-number); }',
    '.hl-literal { color: var(--hl-literal); }',
    '.hl-keyword { color: var(--hl-keyword); }',
    '.hl-comment { color: var(--hl-comment); font-style: italic; }',
    '.hl-tag { color: var(--hl-tag); }',
    '.hl-attr { color: var(--hl-attr); }',
    '.hl-fn { color: var(--hl-fn); }',
    '.hl-meta { color: var(--hl-meta); }',
    '[data-highlight="large"][data-highlight-note]::before { content: attr(data-highlight-note); display: block; font-style: italic; opacity: 0.8; }',
    '.hl-editor { position: relative; }',
    '.hl-editor > textarea { display: block; width: 100%; box-sizing: border-box; resize: vertical; }',
    '.hl-editor > .hl-backdrop { position: absolute; top: 0; left: 0; right: 0; bottom: 0; margin: 0; overflow: hidden; pointer-events: none; white-space: pre-wrap; overflow-wrap: break-word; border-color: transparent; box-sizing: border-box; }',
    '.hl-editor:not([data-highlight="on"]) > .hl-backdrop { visibility: hidden; }',
    '.hl-editor[data-highlight="on"] > textarea { color: transparent; caret-color: var(--hl-caret, CanvasText); }',
  ].join('\n');

  let enabled = true;
  /** @type {Set<() => void>} */
  const editors = new Set();

  // What was coloured before, by language and text: a panel that renders its list again on
  // every poll clones the nodes instead of tokenizing again. Bounded by the characters it holds.
  const CACHE_CHARS = 2_000_000;
  const CACHE_TEXT = 64 * 1024;
  /** @type {Map<string, DocumentFragment>} */
  const painted = new Map();
  let paintedChars = 0;
  /** @type {Map<string, HighlightLang | null>} */
  const detected = new Map();
  const DETECTED_MAX = 1000;

  function ensureStyle() {
    if (document.head.querySelector('style[data-highlight-style]')) return;
    const style = document.createElement('style');
    style.dataset.highlightStyle = '';
    style.textContent = STYLE;
    document.head.append(style);
  }

  /** `ui.syntaxHighlight` of the panel's settings: on unless `false`. Colours every editor
   * again. @param {Record<string, unknown> | null | undefined} settings */
  function configure(settings) {
    enabled = settings?.[SETTING] !== false;
    for (const refresh of editors) refresh();
    return enabled;
  }

  /** `detect`, remembered per text and hint for the texts a panel shows again on every poll.
   * @param {string} text @param {string | null} [hint] */
  function detectCached(text, hint) {
    if (text.length > CACHE_TEXT) return detect(text, hint);
    const key = (hint ?? '') + '\u0000' + text;
    const known = detected.get(key);
    if (known !== undefined) return known;
    const lang = detect(text, hint);
    if (detected.size >= DETECTED_MAX) detected.delete(/** @type {string} */ (detected.keys().next().value));
    detected.set(key, lang);
    return lang;
  }

  /** The language of a value a page answered (the console's and the snippets' results): a
   * string is text, a function JavaScript, anything else JSON when it is JSON; else none.
   * @param {string | undefined} type the value's `typeof` @param {string} text */
  function valueLang(type, text) {
    if (type === 'string') return null;
    if (type === 'function') return 'js';
    return detectCached(text) === 'json' ? 'json' : null;
  }

  /** The nodes of `text` in `lang`: a span per coloured token, the rest - punctuation too - as
   * merged text. @param {string} text @param {HighlightLang} lang */
  function nodes(text, lang) {
    const fragment = document.createDocumentFragment();
    let plain = '';
    for (const token of tokenize(text, lang)) {
      if (token.kind === 'plain' || token.kind === 'punct') {
        plain += token.text;
        continue;
      }
      if (plain) fragment.append(document.createTextNode(plain));
      plain = '';
      const span = document.createElement('span');
      span.className = 'hl-' + token.kind;
      span.textContent = token.text;
      fragment.append(span);
    }
    if (plain) fragment.append(document.createTextNode(plain));
    return fragment;
  }

  /** `nodes`, from the cache when the same text was coloured before. @param {string} text @param {HighlightLang} lang */
  function cachedNodes(text, lang) {
    if (text.length > CACHE_TEXT) return nodes(text, lang);
    const key = lang + '\u0000' + text;
    let fragment = painted.get(key);
    if (!fragment) {
      fragment = nodes(text, lang);
      painted.set(key, fragment);
      paintedChars += key.length;
      for (const [old] of painted) {
        if (paintedChars <= CACHE_CHARS) break;
        painted.delete(old);
        paintedChars -= old.length;
      }
    }
    return /** @type {DocumentFragment} */ (fragment.cloneNode(true));
  }

  /** Shows `text` in `target`, coloured as `lang`. @param {HTMLElement} target @param {string} text
   * @param {HighlightLang | null} lang @param {string} [note] said above a text too large to colour */
  function render(target, text, lang, note) {
    ensureStyle();
    delete target.dataset.highlightNote;
    target.removeAttribute('title');
    /** @type {HighlightOutcome} */
    const outcome = !enabled ? 'off' : !lang ? 'plain' : tooLarge(text) ? 'large' : 'on';
    target.dataset.highlight = outcome;
    if (outcome !== 'on' || !lang) {
      target.textContent = text;
      if (outcome === 'large' && note) {
        target.dataset.highlightNote = note;
        target.title = note;
      }
      return outcome;
    }
    target.replaceChildren(cachedNodes(text, lang));
    return outcome;
  }

  const MIRRORED = ['fontFamily', 'fontSize', 'fontWeight', 'lineHeight', 'letterSpacing', 'tabSize',
    'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
    'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth', 'borderStyle', 'borderRadius'];

  /** Colours `textarea`: its text turns transparent - caret, selection and background stay - and
   * a `<pre>` laid over it, sized to it, shows the same text in colour and scrolls with it.
   * Typing colours once per frame; setting `value`, `defaultValue` or `setRangeText`, a form
   * reset, focus and blur colour again when the text changed. The language a function names is
   * asked again only when the text's first or last character changes or `refresh()` is called.
   * Switched off or above LIMIT_BYTES the `<pre>` is empty; above, the field shows `note()`.
   * @param {HTMLTextAreaElement} textarea
   * @param {HighlightLang | ((text: string) => HighlightLang | null)} lang
   * @param {() => string} [note]
   * @returns {HighlightEditor} */
  function editor(textarea, lang, note) {
    ensureStyle();
    const wrap = document.createElement('div');
    wrap.className = 'hl-editor';
    const backdrop = document.createElement('pre');
    backdrop.className = 'hl-backdrop';
    backdrop.setAttribute('aria-hidden', 'true');
    textarea.replaceWith(wrap);
    wrap.append(textarea, backdrop);
    const proto = HTMLTextAreaElement.prototype;
    const valueOf = /** @type {PropertyDescriptor} */ (Object.getOwnPropertyDescriptor(proto, 'value'));
    const defaultOf = /** @type {PropertyDescriptor} */ (Object.getOwnPropertyDescriptor(proto, 'defaultValue'));
    const text = () => String(valueOf.get?.call(textarea) ?? '');
    /** @type {string | null} */
    let shown = null;
    let queued = false;
    /** @type {HighlightLang | null} */
    let chosen = null;
    let chosenFor = '';
    let fresh = false;

    function sync() {
      backdrop.scrollTop = textarea.scrollTop;
      backdrop.scrollLeft = textarea.scrollLeft;
    }

    /** The textarea's metrics and size on the `<pre>`; its scroll bar's width as padding, so
     * lines wrap alike. Only at the start and when the textarea's size changes. */
    function measure() {
      const style = getComputedStyle(textarea);
      const target = /** @type {Record<string, string>} */ (/** @type {unknown} */ (backdrop.style));
      for (const name of MIRRORED) target[name] = /** @type {Record<string, string>} */ (/** @type {unknown} */ (style))[name] ?? '';
      const bar = textarea.offsetWidth - textarea.clientWidth - (parseFloat(style.borderLeftWidth) || 0) - (parseFloat(style.borderRightWidth) || 0);
      if (bar > 0) backdrop.style.paddingRight = `calc(${style.paddingRight} + ${bar}px)`;
      if (textarea.offsetWidth > 0) {
        backdrop.style.width = textarea.offsetWidth + 'px';
        backdrop.style.height = textarea.offsetHeight + 'px';
      }
      sync();
    }

    /** The language of `value`. @param {string} value */
    function languageOf(value) {
      if (typeof lang !== 'function') return lang;
      const trimmed = value.trim();
      const kind = trimmed.charAt(0) + trimmed.charAt(trimmed.length - 1);
      if (!fresh || kind !== chosenFor) {
        chosen = lang(value);
        chosenFor = kind;
        fresh = true;
      }
      return chosen;
    }

    /** Colours the text when it changed since the last time, or always with `force`.
     * @param {boolean} force */
    function paint(force) {
      queued = false;
      const value = text();
      if (!force && value === shown) return;
      shown = value;
      const large = tooLarge(value);
      const language = enabled && !large ? languageOf(value) : null;
      /** @type {HighlightOutcome} */
      const outcome = !enabled ? 'off' : large ? 'large' : language ? 'on' : 'plain';
      wrap.dataset.highlight = outcome;
      // The trailing newline keeps a last empty line as tall as the textarea's.
      if (language && outcome === 'on' && value) backdrop.replaceChildren(nodes(value + '\n', language));
      else backdrop.replaceChildren();
      const said = outcome === 'large' && note ? note() : '';
      if (said) wrap.dataset.highlightNote = said;
      else delete wrap.dataset.highlightNote;
      sync();
    }

    function schedule() {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => paint(false));
    }

    function refresh() {
      fresh = false;
      paint(true);
    }

    // The panels set their fields from code: `value`, `defaultValue` and `setRangeText` colour
    // again, as typing does.
    Object.defineProperty(textarea, 'value', {
      configurable: true,
      get: () => valueOf.get?.call(textarea),
      set: (next) => {
        valueOf.set?.call(textarea, next);
        paint(false);
      },
    });
    Object.defineProperty(textarea, 'defaultValue', {
      configurable: true,
      get: () => defaultOf.get?.call(textarea),
      set: (next) => {
        defaultOf.set?.call(textarea, next);
        paint(false);
      },
    });
    const setRangeText = proto.setRangeText;
    /** @param {...unknown} args */
    textarea.setRangeText = (...args) => {
      Reflect.apply(setRangeText, textarea, args);
      paint(false);
    };
    textarea.addEventListener('input', schedule);
    textarea.addEventListener('focus', () => paint(false));
    textarea.addEventListener('blur', () => paint(false));
    textarea.form?.addEventListener('reset', () => setTimeout(() => paint(false)));
    textarea.addEventListener('scroll', sync);
    // Resized by hand or by the panel: the metrics and the size again, the colours stay.
    if (typeof ResizeObserver === 'function') new ResizeObserver(() => measure()).observe(textarea);
    editors.add(refresh);
    measure();
    paint(true);
    return { refresh };
  }

  return Object.freeze({
    LIMIT_BYTES,
    LANGS,
    SETTING,
    get enabled() {
      return enabled;
    },
    tokenize,
    detect: detectCached,
    tooLarge,
    valueLang,
    configure,
    render,
    editor,
  });
})();
