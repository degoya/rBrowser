// The first-party plugin panels' texts (RB-080-59): the plugin's strings from the plugin panels
// window, a key's text with its `{name}` parameters filled, and the request for them. Until they
// arrived the panel shows nothing (`<body data-texts="waiting">` hides every child), so a raw key
// never shows; the request goes out again every second, and after ten the log says so and the
// panel shows one English line with a Retry button. A key the locale lacks is logged once
// (plugins/web-common/src/panel-keys.test.ts keeps that from shipping).
//
// RB-080-90: a text is a message of the catalogue format - `{name}` and the ICU plural
// `{n, plural, =0 {...} one {# view} other {# views}}` - as the core's (rb-i18n) and the shell's
// (ui/src/i18n/format.ts); `PanelTexts.format` fills it. A failed call is one sentence,
// `PanelTexts.errorText`: the text of its code (the plugin's own, or a host code's, which the
// panels window sends along) and the text of its `reason` - never the code itself.
//
// One source, this file. A panel's files lie in its own package and a classic script cannot
// import: rulers-guides and screenshot-editor ship a verbatim copy as `panel/texts.js` (loaded
// before their panel.js), simulators bundles this file; plugins/web-common/src/panel-texts.test.ts
// holds the copies to it. A classic script: `PanelTexts` is a global.
globalThis.PanelTexts = (() => {
  'use strict';
  const ASK_EVERY_MS = 1000;
  const ASK_TRIES = 10;
  // English on purpose: the one line a panel shows when its texts never came.
  const NOT_ARRIVED = "The panel's texts did not arrive.";
  const RETRY = 'Retry';
  const HIDE = 'body[data-texts="waiting"] > :not([data-texts-notice]) { display: none !important; }';

  // English on purpose: a failure whose code has no text and no host text came along.
  const FAILED = 'The plugin could not do that.';
  const NAME = /[A-Za-z_][A-Za-z0-9_]*/y;

  /** The parts of message `src`: text, `{arg}`, plurals `{arg, branches}` and `#` (`hash`).
   * Throws where it does not parse. @param {string} src */
  function parse(src) {
    let pos = 0;
    const fail = () => {
      throw new SyntaxError(`the message does not parse at offset ${pos}`);
    };
    const space = () => {
      while (/\s/.test(src[pos] ?? '')) pos += 1;
    };
    const name = () => {
      NAME.lastIndex = pos;
      const found = NAME.exec(src);
      if (!found) fail();
      pos = NAME.lastIndex;
      return /** @type {RegExpExecArray} */ (found)[0];
    };
    const expect = (/** @type {string} */ c) => {
      if (src[pos] !== c) fail();
      pos += 1;
    };
    /** @param {boolean} inBranch @returns {any[]} */
    function message(inBranch) {
      /** @type {any[]} */
      const parts = [];
      let text = '';
      const flush = () => {
        if (text) parts.push(text);
        text = '';
      };
      while (pos < src.length) {
        const c = src[pos];
        if (c === '}') {
          if (!inBranch) fail();
          break;
        }
        if (c === '#' && inBranch) {
          flush();
          parts.push({ hash: true });
          pos += 1;
        } else if (c !== '{') {
          text += c;
          pos += 1;
        } else {
          flush();
          pos += 1;
          space();
          const arg = name();
          space();
          if (src[pos] === '}') {
            pos += 1;
            parts.push({ arg });
            continue;
          }
          expect(',');
          space();
          if (name() !== 'plural') fail();
          space();
          expect(',');
          /** @type {{ selector: string, body: any[] }[]} */
          const branches = [];
          for (;;) {
            space();
            if (src[pos] === '}') break;
            let selector;
            if (src[pos] === '=') {
              const digits = /^[0-9]+/.exec(src.slice(pos + 1));
              if (!digits) fail();
              selector = `=${/** @type {RegExpExecArray} */ (digits)[0]}`;
              pos += selector.length;
            } else {
              selector = name();
            }
            space();
            expect('{');
            const body = message(true);
            expect('}');
            branches.push({ selector, body });
          }
          expect('}');
          if (!branches.some((branch) => branch.selector === 'other')) fail();
          parts.push({ arg, branches });
        }
      }
      flush();
      return parts;
    }
    return message(false);
  }

  /** @param {any[]} parts @param {Record<string, unknown>} params @param {string} [hash] @returns {string} */
  function render(parts, params, hash) {
    return parts
      .map((part) => {
        if (typeof part === 'string') return part;
        if (part.hash) return hash ?? '#';
        const value = String(params[part.arg] ?? '');
        if (!part.branches) return value;
        const n = Number(value);
        // English (phase 1): `one` for 1, else `other`; an exact `=n` first.
        const branch =
          part.branches.find((/** @type {any} */ entry) => entry.selector === `=${n}`) ??
          part.branches.find((/** @type {any} */ entry) => entry.selector === (n === 1 ? 'one' : 'other')) ??
          part.branches.find((/** @type {any} */ entry) => entry.selector === 'other');
        return render(branch.body, params, value);
      })
      .join('');
  }

  /** Message `template` with its placeholders and plurals filled from `params`; as written when
   * it does not parse. @param {string} template @param {Record<string, unknown>} [params] */
  function format(template, params = {}) {
    try {
      return render(parse(template), params);
    } catch {
      return template;
    }
  }

  /** A failed call as one sentence: the text of `code` (the plugin's or the host's) and the
   * text of its `reason`; a code without a text is said generically, never shown.
   * @param {Record<string, string>} strings @param {string} code @param {Record<string, unknown>} [params] */
  function errorText(strings, code, params = {}) {
    const own = strings[code];
    const base = own !== undefined ? format(own, params) : (strings['plugins.panel.call_failed'] ?? FAILED);
    const reason = typeof params.reason === 'string' ? params.reason : '';
    const said = strings[`plugins.reason.${reason}`];
    // A reason without a text is shown only when it is words (an engine's message), not a token.
    const because = said !== undefined ? format(said, params) : /^[a-z0-9_]*$/.test(reason) ? '' : reason.slice(0, 200);
    return because ? `${base} ${because}` : base;
  }

  const fill = format;

  /** The texts of the plugin whose keys start with `prefix`; `name` names the panel in the log.
   * Hides the panel until they arrive. @param {string} prefix @param {string} name */
  function create(prefix, name) {
    /** @type {Record<string, string>} */
    let strings = {};
    let ready = false;
    /** @type {Set<string>} */
    const warned = new Set();
    /** @type {HTMLElement | null} */
    let notice = null;
    const style = document.createElement('style');
    style.textContent = HIDE;
    document.head.append(style);
    document.body.dataset.texts = 'waiting';

    function giveUp() {
      console.warn(`${name}: no texts from the plugin panels window`);
      notice = document.createElement('p');
      notice.setAttribute('role', 'alert');
      notice.dataset.textsNotice = '';
      notice.append(NOT_ARRIVED, ' ');
      const retry = document.createElement('button');
      retry.type = 'button';
      retry.textContent = RETRY;
      retry.addEventListener('click', () => {
        notice?.remove();
        notice = null;
        ask(1);
      });
      notice.append(retry);
      document.body.append(notice);
    }

    /** @param {number} tries */
    function ask(tries) {
      if (ready) return;
      window.parent.postMessage({ type: 'settings' }, '*');
      setTimeout(() => {
        if (ready) return;
        if (tries < ASK_TRIES) ask(tries + 1);
        else giveUp();
      }, ASK_EVERY_MS);
    }

    /** Logs a full key the texts lack, once and only once they are there. @param {string} key */
    function missing(key) {
      if (!ready || warned.has(key)) return;
      warned.add(key);
      console.warn(`${name}: no text for ${key}`);
    }

    return {
      /** Whether the texts arrived. */
      get ready() {
        return ready;
      },
      /** The texts by full key. */
      get strings() {
        return strings;
      },
      /** Asks the panels window for the texts until they arrive. */
      ask: () => ask(1),
      /** Takes the texts and shows the panel; `true` the first time. @param {unknown} next */
      take(next) {
        strings = next !== null && typeof next === 'object' ? /** @type {Record<string, string>} */ (next) : {};
        const first = !ready;
        ready = true;
        notice?.remove();
        notice = null;
        delete document.body.dataset.texts;
        return first;
      },
      missing,
      /** The text of a full key (an error code), or `undefined`. @param {string} key */
      full: (key) => strings[key],
      /** A failed call as a sentence (`errorText`). @param {string} code @param {Record<string, unknown>} [params] */
      error: (code, params = {}) => errorText(strings, code, params),
      /** The text of `key` below the prefix. @param {string} key @param {Record<string, unknown>} [params] */
      text(key, params = {}) {
        const template = strings[prefix + key];
        if (template !== undefined) return fill(template, params);
        missing(prefix + key);
        return fill(key, params);
      },
    };
  }

  return { create, format, errorText };
})();
