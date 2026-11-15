// @ts-check
// The tokenizers of the first-party plugin panels' syntax highlighter (RB-080-65; highlight.js
// shows their tokens): JSON, JavaScript, HTML, XML, CSS, HTTP headers and stack traces, offline
// and without a library. Each splits a text into tokens whose texts joined are the text again -
// nothing is lost, and an unterminated string, comment or tag ends at the end of the text (a
// "..." of JavaScript at its line). Every step consumes text, so any input ends; a long line of
// headers or a stack stays plain, where its patterns would backtrack.
//
// One source, this file. The classic-script panels that load it (console, network-panel,
// snippets, storage-panel) list it in their `web-common.txt`, and scripts/build-plugins.sh packs
// it into their panel/ (scripts/lib/plugin-stage.sh); their tests load it from here. No copies:
// plugins/web-common/src/highlight.test.ts holds the lists and pages to it. A classic script:
// `CodeTokens` is a global.
globalThis.CodeTokens = (() => {
  'use strict';
  const LIMIT_BYTES = 200 * 1024;
  /** @type {readonly HighlightLang[]} */
  const LANGS = Object.freeze(['json', 'js', 'html', 'xml', 'css', 'http', 'stack']);

  /** Adds a token; plain text joins the plain text before it.
   * @param {HighlightToken[]} tokens @param {HighlightKind} kind @param {string} text */
  function push(tokens, kind, text) {
    if (!text) return;
    const last = tokens[tokens.length - 1];
    if (kind === 'plain' && last?.kind === 'plain') last.text += text;
    else tokens.push({ kind, text });
  }

  /** The match of the sticky `re` at `from`, or ''. @param {RegExp} re @param {string} text @param {number} from */
  function at(re, text, from) {
    re.lastIndex = from;
    const found = re.exec(text);
    return found ? found[0] : '';
  }

  /** The end of the string opened by the quote at `start`: after its closing quote, at a line
   * end when `line`, else at the text's end. @param {string} text @param {number} start @param {boolean} line */
  function quoted(text, start, line) {
    const quote = text.charAt(start);
    let i = start + 1;
    while (i < text.length) {
      const c = text.charAt(i);
      if (c === '\\') i += 2;
      else if (c === quote) return i + 1;
      else if (line && c === '\n') return i;
      else i += 1;
    }
    return text.length;
  }

  /** The index after `close` from `from`, or the text's end. @param {string} text @param {number} from @param {string} close */
  function until(text, from, close) {
    const end = text.indexOf(close, from);
    return end < 0 ? text.length : end + close.length;
  }

  const SPACE = /\s+/y;
  const COLON_NEXT = /\s*:/y;
  const JSON_NUMBER = /-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/y;
  const JSON_WORD = /(?:true|false|null)\b/y;

  /** @param {string} text @returns {HighlightToken[]} */
  function json(text) {
    /** @type {HighlightToken[]} */
    const tokens = [];
    let i = 0;
    while (i < text.length) {
      const c = text.charAt(i);
      let m = '';
      if (c === '"') {
        const end = quoted(text, i, false);
        push(tokens, at(COLON_NEXT, text, end) ? 'key' : 'string', text.slice(i, end));
        i = end;
      } else if ('{}[]:,'.includes(c)) {
        push(tokens, 'punct', c);
        i += 1;
      } else if ((m = at(SPACE, text, i))) {
        push(tokens, 'plain', m);
        i += m.length;
      } else if ((m = at(JSON_NUMBER, text, i))) {
        push(tokens, 'number', m);
        i += m.length;
      } else if ((m = at(JSON_WORD, text, i))) {
        push(tokens, 'literal', m);
        i += m.length;
      } else {
        push(tokens, 'plain', c);
        i += 1;
      }
    }
    return tokens;
  }

  const JS_KEYWORDS = new Set(('await break case catch class const continue debugger default delete do else export extends ' +
    'finally for function if import in instanceof let new of return static super switch this throw try typeof var void ' +
    'while with yield async').split(' '));
  const JS_LITERALS = new Set(['true', 'false', 'null', 'undefined', 'NaN', 'Infinity']);
  const JS_IDENT = /[A-Za-z_$][\w$]*/y;
  const JS_NUMBER = /(?:0[xX][\da-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\d[\d_]*(?:\.[\d_]*)?(?:[eE][+-]?\d+)?|\.\d[\d_]*(?:[eE][+-]?\d+)?)n?/y;
  const CALL_NEXT = /\s*\(/y;
  const JS_PUNCT = '{}[]()<>;,.:?!=+-*/%&|^~';

  /** JavaScript; a regular expression literal is not told from a division. @param {string} text @returns {HighlightToken[]} */
  function js(text) {
    /** @type {HighlightToken[]} */
    const tokens = [];
    let i = 0;
    // The last character that was not space: a key follows `{` or `,` and precedes `:`.
    let before = '';
    while (i < text.length) {
      const c = text.charAt(i);
      const two = text.slice(i, i + 2);
      let m = '';
      if (two === '//') {
        const end = text.indexOf('\n', i);
        m = text.slice(i, end < 0 ? text.length : end);
        push(tokens, 'comment', m);
      } else if (two === '/*') {
        m = text.slice(i, until(text, i + 2, '*/'));
        push(tokens, 'comment', m);
      } else if (c === '"' || c === "'" || c === '`') {
        m = text.slice(i, quoted(text, i, c !== '`'));
        const key = (before === '{' || before === ',') && at(COLON_NEXT, text, i + m.length);
        push(tokens, key ? 'key' : 'string', m);
      } else if ((m = at(SPACE, text, i))) {
        push(tokens, 'plain', m);
        i += m.length;
        continue;
      } else if ((m = at(JS_NUMBER, text, i))) {
        push(tokens, 'number', m);
      } else if ((m = at(JS_IDENT, text, i))) {
        const next = i + m.length;
        /** @type {HighlightKind} */
        let kind = 'plain';
        if (JS_KEYWORDS.has(m)) kind = 'keyword';
        else if (JS_LITERALS.has(m)) kind = 'literal';
        else if ((before === '{' || before === ',') && at(COLON_NEXT, text, next)) kind = 'key';
        else if (at(CALL_NEXT, text, next)) kind = 'fn';
        push(tokens, kind, m);
      } else {
        m = c;
        push(tokens, JS_PUNCT.includes(c) ? 'punct' : 'plain', c);
      }
      i += m.length;
      before = m.charAt(m.length - 1);
    }
    return tokens;
  }

  const CSS_IDENT = /-?-?[A-Za-z_][\w-]*/y;
  const CSS_AT = /@[\w-]+/y;
  const CSS_NUMBER = /[+-]?(?:\d+\.?\d*|\.\d+)(?:%|[A-Za-z]+)?/y;
  const CSS_HASH = /#[\w-]+/y;
  const CSS_IMPORTANT = /!\s*important\b/iy;
  const CSS_TO_END = /[^{};]*/y;

  /** CSS. @param {string} text @returns {HighlightToken[]} */
  function css(text) {
    /** @type {HighlightToken[]} */
    const tokens = [];
    let i = 0;
    let depth = 0;
    // Where a statement starts, inside a declaration's value, and whether this statement is one.
    let start = true;
    let value = false;
    let declaration = false;
    while (i < text.length) {
      const c = text.charAt(i);
      let m = '';
      if (text.startsWith('/*', i)) {
        m = text.slice(i, until(text, i + 2, '*/'));
        push(tokens, 'comment', m);
        i += m.length;
        continue;
      }
      if ((m = at(SPACE, text, i))) {
        push(tokens, 'plain', m);
        i += m.length;
        continue;
      }
      if (start) {
        // A statement inside a block is a declaration unless a block follows it (a nested rule).
        const rest = at(CSS_TO_END, text, i);
        declaration = depth > 0 && text.charAt(i + rest.length) !== '{' && rest.includes(':');
        start = false;
      }
      if (c === '"' || c === "'") {
        m = text.slice(i, quoted(text, i, true));
        push(tokens, 'string', m);
      } else if (c === '{' || c === '}' || c === ';') {
        m = c;
        depth = c === '{' ? depth + 1 : c === '}' ? Math.max(depth - 1, 0) : depth;
        start = true;
        value = false;
        push(tokens, 'punct', c);
      } else if ((m = at(CSS_AT, text, i))) {
        push(tokens, 'keyword', m);
      } else if ((m = at(CSS_IMPORTANT, text, i))) {
        push(tokens, 'keyword', m);
      } else if ((m = at(CSS_HASH, text, i))) {
        push(tokens, declaration ? 'number' : 'tag', m);
      } else if ((m = at(CSS_NUMBER, text, i))) {
        push(tokens, 'number', m);
      } else if ((m = at(CSS_IDENT, text, i))) {
        /** @type {HighlightKind} */
        let kind = 'tag';
        const last = tokens[tokens.length - 1]?.text ?? '';
        if (at(CALL_NEXT, text, i + m.length)) kind = 'fn';
        else if (declaration) kind = value ? 'literal' : 'key';
        else if (last.endsWith(':')) kind = 'plain';
        else if (last.endsWith('(') && at(COLON_NEXT, text, i + m.length)) kind = 'key';
        push(tokens, kind, m);
      } else {
        m = c;
        if (c === ':' && declaration) value = true;
        push(tokens, '()[],:>+~*.='.includes(c) ? 'punct' : 'plain', c);
      }
      i += m.length;
    }
    return tokens;
  }

  const TAG_OPEN = /<\/?[A-Za-z][\w:.-]*/y;
  const ATTR_NAME = /[^\s"'<>/=]+/y;
  const BARE_VALUE = /[^\s"'<>]+/y;
  const ENTITY = /&(?:#\d+|#[xX][\da-fA-F]+|\w+);/y;
  const TEXT = /[^<&]+/y;

  /** HTML or XML; in HTML a script's and a style's content in its own language.
   * @param {string} text @param {boolean} html @returns {HighlightToken[]} */
  function markup(text, html) {
    /** @type {HighlightToken[]} */
    const tokens = [];
    let i = 0;
    while (i < text.length) {
      let m = '';
      if (text.startsWith('<!--', i)) {
        m = text.slice(i, until(text, i + 4, '-->'));
        push(tokens, 'comment', m);
      } else if (text.startsWith('<![CDATA[', i)) {
        m = text.slice(i, until(text, i + 9, ']]>'));
        push(tokens, 'string', m);
      } else if (text.startsWith('<!', i) || text.startsWith('<?', i)) {
        m = text.slice(i, until(text, i + 2, '>'));
        push(tokens, 'meta', m);
      } else if ((m = at(TAG_OPEN, text, i))) {
        push(tokens, 'tag', m);
        const name = m.slice(1).toLowerCase();
        i = attributes(text, i + m.length, tokens);
        const closed = text.charAt(i - 1) === '>' && text.charAt(i - 2) !== '/';
        if (html && closed && (name === 'script' || name === 'style')) {
          const close = new RegExp('</' + name, 'ig');
          close.lastIndex = i;
          const end = close.exec(text)?.index ?? text.length;
          const inner = text.slice(i, end);
          for (const token of name === 'script' ? js(inner) : css(inner)) push(tokens, token.kind, token.text);
          i += inner.length;
        }
        continue;
      } else if ((m = at(ENTITY, text, i))) {
        push(tokens, 'literal', m);
      } else {
        m = at(TEXT, text, i) || text.charAt(i);
        push(tokens, 'plain', m);
      }
      i += m.length;
    }
    return tokens;
  }

  /** A tag's attributes from `i` up to and with its `>`; the index after.
   * @param {string} text @param {number} i @param {HighlightToken[]} tokens */
  function attributes(text, i, tokens) {
    let afterEquals = false;
    while (i < text.length) {
      const c = text.charAt(i);
      let m = '';
      if (c === '>' || text.startsWith('/>', i)) {
        m = c === '>' ? '>' : '/>';
        push(tokens, 'tag', m);
        return i + m.length;
      }
      if (c === '<') return i;
      if ((m = at(SPACE, text, i))) {
        push(tokens, 'plain', m);
        i += m.length;
        continue;
      }
      if (c === '"' || c === "'") {
        m = text.slice(i, quoted(text, i, false));
        push(tokens, 'string', m);
      } else if (c === '=') {
        m = c;
        push(tokens, 'punct', c);
      } else if (afterEquals && (m = at(BARE_VALUE, text, i))) {
        push(tokens, 'string', m);
      } else if ((m = at(ATTR_NAME, text, i))) {
        push(tokens, 'attr', m);
      } else {
        m = c;
        push(tokens, 'punct', c);
      }
      afterEquals = m === '=';
      i += m.length;
    }
    return i;
  }

  // A longer line of headers or a stack stays plain: its patterns backtrack.
  const LONG_LINE = 4096;
  /** The lines of `text` with their line ends. @param {string} text */
  const lines = (text) => text.match(/[^\n]*\n|[^\n]+$/g) ?? [];

  const STATUS_LINE = /^(HTTP\/[\d.]+)(\s+)(\d{3})(.*)$/;
  const REQUEST_LINE = /^([A-Z]+)(\s+)(\S+)(\s+)(HTTP\/[\d.]+)(\s*)$/;
  const HEADER_LINE = /^(:?[!#$%&'*+.^_`|~0-9A-Za-z-]+)(\s*:)(.*)$/;

  /** HTTP headers, with a status or request line. @param {string} text @returns {HighlightToken[]} */
  function http(text) {
    /** @type {HighlightToken[]} */
    const tokens = [];
    for (const whole of lines(text)) {
      const line = whole.replace(/\r?\n$/, '');
      if (line.length > LONG_LINE) {
        push(tokens, 'plain', whole);
        continue;
      }
      /** @type {RegExpMatchArray | null} */
      let m = null;
      /** @type {HighlightKind[]} */
      let kinds = [];
      if ((m = STATUS_LINE.exec(line))) kinds = ['keyword', 'plain', 'number', 'plain'];
      else if ((m = REQUEST_LINE.exec(line))) kinds = ['keyword', 'plain', 'string', 'plain', 'keyword', 'plain'];
      else if ((m = HEADER_LINE.exec(line))) kinds = ['key', 'punct', 'string'];
      if (m) kinds.forEach((kind, index) => push(tokens, kind, m?.[index + 1] ?? ''));
      else push(tokens, 'plain', line);
      push(tokens, 'plain', whole.slice(line.length));
    }
    return tokens;
  }

  const ERROR_LINE = /^(\w*(?:Error|Exception)|Uncaught)(:?)(.*)$/;
  const V8_FRAME = /^(\s*)(at)(\s+)(.*)$/;
  const NAMED_PLACE = /^(.*?)(\s+\()(.*)(\))(\s*)$/;
  const FIREFOX_FRAME = /^([^@\s]*)(@)(.*)$/;
  const PLACE = /^(.*?)(:\d+)(:\d+)?(\s*)$/;

  /** A frame's place, `url:line:column`. @param {string} where @param {HighlightToken[]} tokens */
  function place(where, tokens) {
    const m = PLACE.exec(where);
    if (!m) return push(tokens, 'string', where);
    push(tokens, 'string', m[1] ?? '');
    push(tokens, 'number', (m[2] ?? '') + (m[3] ?? ''));
    push(tokens, 'plain', m[4] ?? '');
  }

  /** A stack trace of V8 (`at f (url:1:2)`) or Firefox and WebKit (`f@url:1:2`).
   * @param {string} text @returns {HighlightToken[]} */
  function stack(text) {
    /** @type {HighlightToken[]} */
    const tokens = [];
    for (const whole of lines(text)) {
      const line = whole.replace(/\r?\n$/, '');
      if (line.length > LONG_LINE) {
        push(tokens, 'plain', whole);
        continue;
      }
      let m = V8_FRAME.exec(line);
      if (m) {
        push(tokens, 'plain', m[1] ?? '');
        push(tokens, 'keyword', m[2] ?? '');
        push(tokens, 'plain', m[3] ?? '');
        const rest = m[4] ?? '';
        const named = NAMED_PLACE.exec(rest);
        if (named) {
          push(tokens, 'fn', named[1] ?? '');
          push(tokens, 'punct', named[2] ?? '');
          place(named[3] ?? '', tokens);
          push(tokens, 'punct', named[4] ?? '');
          push(tokens, 'plain', named[5] ?? '');
        } else place(rest, tokens);
      } else if ((m = FIREFOX_FRAME.exec(line))) {
        push(tokens, 'fn', m[1] ?? '');
        push(tokens, 'punct', m[2] ?? '');
        place(m[3] ?? '', tokens);
      } else if ((m = ERROR_LINE.exec(line))) {
        push(tokens, 'keyword', m[1] ?? '');
        push(tokens, 'punct', m[2] ?? '');
        push(tokens, 'plain', m[3] ?? '');
      } else push(tokens, 'plain', line);
      push(tokens, 'plain', whole.slice(line.length));
    }
    return tokens;
  }

  /** The tokens of `text` in `lang`; joined, they are `text`. @param {string} text @param {HighlightLang} lang */
  function tokenize(text, lang) {
    switch (lang) {
      case 'json': return json(text);
      case 'js': return js(text);
      case 'html': return markup(text, true);
      case 'xml': return markup(text, false);
      case 'css': return css(text);
      case 'http': return http(text);
      case 'stack': return stack(text);
      default: return text ? [{ kind: /** @type {HighlightKind} */ ('plain'), text }] : [];
    }
  }

  /** Whether `text` is above LIMIT_BYTES as UTF-8. @param {string} text */
  function tooLarge(text) {
    // A UTF-16 unit is at most three bytes: only a long text is measured.
    return text.length * 3 > LIMIT_BYTES && new TextEncoder().encode(text).length > LIMIT_BYTES;
  }

  /** The language of `hint` (a language or a MIME type), else of the text: JSON that parses,
   * markup that starts with `<`; null for anything else. @param {string} text @param {string | null} [hint] */
  function detect(text, hint) {
    const type = String(hint ?? '').toLowerCase().split(';')[0]?.trim() ?? '';
    const named = LANGS.find((lang) => lang === type);
    if (named) return named;
    if (/json/.test(type)) return 'json';
    if (/html/.test(type)) return 'html';
    if (/xml|svg/.test(type)) return 'xml';
    if (/css/.test(type)) return 'css';
    if (/javascript|ecmascript/.test(type)) return 'js';
    const trimmed = text.trim();
    if (!trimmed || tooLarge(trimmed)) return null;
    if (trimmed.startsWith('<')) return /^<\?xml/i.test(trimmed) ? 'xml' : 'html';
    try {
      JSON.parse(trimmed);
      return 'json';
    } catch {
      return null;
    }
  }

  return Object.freeze({ LIMIT_BYTES, LANGS, tokenize, detect, tooLarge });
})();
