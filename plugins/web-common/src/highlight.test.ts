/// <reference path="../panel/highlight.d.ts" />
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, it } from 'vitest';
import { runClassicScripts } from './harness';

// RB-080-65: the one syntax highlighter of the first-party panels. Page content is foreign, so
// the text only ever becomes text nodes and spans - never markup.

const PLUGINS = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
/** The two files of the highlighter, in the order a panel loads them. */
const FILES = ['highlight-tokens.js', 'highlight.js'];
const sources = FILES.map((file) => readFileSync(join(PLUGINS, 'web-common', 'panel', file), 'utf8'));

/** The panels that bundle it: on Nuxt UI, through panel-ui's src/code (RB-080-84). */
const HIGHLIGHT_PANELS = ['console', 'coverage', 'error-report', 'network-panel', 'snippets', 'storage-panel'];

const kinds = (text: string, lang: HighlightLang): string[] =>
  CodeHighlight.tokenize(text, lang)
    .filter((token) => token.kind !== 'plain')
    .map((token) => `${token.kind}:${token.text}`);

beforeEach(() => {
  document.head.innerHTML = '';
  document.body.innerHTML = '<pre id="out"></pre>';
  runClassicScripts(sources);
  CodeHighlight.configure({});
});

const out = (): HTMLElement => document.getElementById('out') as HTMLElement;

describe('the tokenizers', () => {
  const samples: Record<HighlightLang, string> = {
    json: '{\n  "a": [1, -2.5e3, true, null],\n  "b": "x\\"y"\n}\n',
    js: 'const f = async (a) => { /* c */ return `t${a}` + "s" + 0x1f; } // end\nf({ k: 1 });',
    html: '<!doctype html>\n<p class="a" hidden>Hi &amp; <b>you</b><!-- c --></p><script>let x = "</p>";</script><style>a { color: red; }</style>',
    xml: '<?xml version="1.0"?><root a=\'1\'><![CDATA[ <x> ]]><leaf/></root>',
    css: '@media (min-width: 40em) {\n  a:hover, .b > #c { color: #fff !important; margin: 0 1.5rem; background: url("x.png"); }\n}\n/* end */',
    http: 'HTTP/1.1 200 OK\ncontent-type: application/json\nSet-Cookie: a=b; Path=/\n',
    stack: 'TypeError: x is not a function\n    at f (https://a.test/app.js:10:5)\n    at https://a.test/app.js:3:1\ng@https://a.test/b.js:2:7',
  };

  it.each(Object.entries(samples))('%s loses nothing: the tokens joined are the text', (lang, text) => {
    const tokens = CodeHighlight.tokenize(text, lang as HighlightLang);
    expect(tokens.map((token) => token.text).join('')).toBe(text);
    expect(tokens.every((token) => token.text.length > 0)).toBe(true);
  });

  it('JSON: keys, strings, numbers, literals and punctuation', () => {
    expect(kinds('{"a": "b", "n": -1.5e2, "t": true, "z": null}', 'json')).toEqual([
      'punct:{', 'key:"a"', 'punct::', 'string:"b"', 'punct:,', 'key:"n"', 'punct::', 'number:-1.5e2', 'punct:,',
      'key:"t"', 'punct::', 'literal:true', 'punct:,', 'key:"z"', 'punct::', 'literal:null', 'punct:}',
    ]);
  });

  it('JavaScript: keywords, functions, comments, strings, numbers, literals and object keys', () => {
    const found = kinds('if (x) return foo(1, "a", undefined); // c\n/* d */ ({ k: 2 })', 'js');
    expect(found).toContain('keyword:if');
    expect(found).toContain('keyword:return');
    expect(found).toContain('fn:foo');
    expect(found).toContain('number:1');
    expect(found).toContain('string:"a"');
    expect(found).toContain('literal:undefined');
    expect(found).toContain('comment:// c');
    expect(found).toContain('comment:/* d */');
    expect(found).toContain('key:k');
  });

  it('HTML: tags, attributes, values, entities, comments and the script and style inside', () => {
    const found = kinds(samples.html, 'html');
    expect(found).toContain('meta:<!doctype html>');
    expect(found).toContain('tag:<p');
    expect(found).toContain('attr:class');
    expect(found).toContain('string:"a"');
    expect(found).toContain('attr:hidden');
    expect(found).toContain('literal:&amp;');
    expect(found).toContain('comment:<!-- c -->');
    expect(found).toContain('keyword:let');
    expect(found).toContain('string:"</p>"');
    expect(found).toContain('tag:</script');
    expect(found).toContain('key:color');
  });

  it('CSS: at-rules, selectors, properties, values, colours and !important', () => {
    const found = kinds(samples.css, 'css');
    expect(found).toContain('keyword:@media');
    expect(found).toContain('tag:a');
    expect(found).toContain('tag:#c');
    expect(found).toContain('key:color');
    expect(found).toContain('number:#fff');
    expect(found).toContain('keyword:!important');
    expect(found).toContain('number:1.5rem');
    expect(found).toContain('fn:url');
    expect(found).toContain('comment:/* end */');
    // `min-width` inside the at-rule's parentheses and `margin` are properties; `hover` is not.
    expect(found).toContain('key:min-width');
    expect(found).toContain('key:margin');
    expect(found).not.toContain('key:hover');
  });

  it('HTTP headers: the status line and each name and value', () => {
    expect(kinds('HTTP/1.1 404 Not Found\nx-a: 1\n:authority: a.test', 'http')).toEqual([
      'keyword:HTTP/1.1', 'number:404', 'key:x-a', 'punct::', 'string: 1', 'key::authority', 'punct::', 'string: a.test',
    ]);
    expect(kinds('GET /a?b=1 HTTP/2', 'http')).toEqual(['keyword:GET', 'string:/a?b=1', 'keyword:HTTP/2']);
  });

  it('stacks: the error, the frames, their places and lines', () => {
    const found = kinds(samples.stack, 'stack');
    expect(found).toContain('keyword:TypeError');
    expect(found).toContain('keyword:at');
    expect(found).toContain('fn:f');
    expect(found).toContain('string:https://a.test/app.js');
    expect(found).toContain('number::10:5');
    expect(found).toContain('fn:g');
  });

  it('end on unterminated strings, comments, tags and CDATA without losing text', () => {
    const cases: Array<[string, HighlightLang]> = [
      ['{"a": "never closed', 'json'],
      ['let s = "open\nnext line', 'js'],
      ['let t = `open template', 'js'],
      ['/* open comment', 'js'],
      ['<a href="x', 'html'],
      ['<!-- open', 'html'],
      ['<script>let a = 1', 'html'],
      ['<![CDATA[ open', 'xml'],
      ['a { color: "open', 'css'],
      ['x-a', 'http'],
      ['    at (', 'stack'],
    ];
    for (const [text, lang] of cases) {
      expect(CodeHighlight.tokenize(text, lang).map((token) => token.text).join(''), `${lang}: ${text}`).toBe(text);
    }
    // An unterminated "..." in JavaScript ends at its line: the next line is code again.
    expect(kinds('let s = "open\nreturn', 'js')).toContain('keyword:return');
  });

  it('take any text, of any language, in linear steps', () => {
    const noise = '<"\'`/*{[(:;@#&!-\\\né\u{1F600}'.repeat(2000);
    for (const lang of CodeHighlight.LANGS) {
      expect(CodeHighlight.tokenize(noise, lang).map((token) => token.text).join('')).toBe(noise);
    }
  });
});

describe('detect', () => {
  it('names the language of a MIME type', () => {
    expect(CodeHighlight.detect('', 'application/json; charset=utf-8')).toBe('json');
    expect(CodeHighlight.detect('', 'application/ld+json')).toBe('json');
    expect(CodeHighlight.detect('', 'text/html')).toBe('html');
    expect(CodeHighlight.detect('', 'image/svg+xml')).toBe('xml');
    expect(CodeHighlight.detect('', 'text/css')).toBe('css');
    expect(CodeHighlight.detect('', 'text/javascript')).toBe('js');
    expect(CodeHighlight.detect('', 'application/javascript')).toBe('js');
    expect(CodeHighlight.detect('', 'css')).toBe('css');
  });

  it('looks at the text without a known type', () => {
    expect(CodeHighlight.detect('  {"a": 1}\n')).toBe('json');
    expect(CodeHighlight.detect('"text"')).toBe('json');
    expect(CodeHighlight.detect('42', 'text/plain')).toBe('json');
    expect(CodeHighlight.detect('<html><body></body></html>')).toBe('html');
    expect(CodeHighlight.detect('<?xml version="1.0"?><a/>')).toBe('xml');
    expect(CodeHighlight.detect('Hello, world')).toBeNull();
    expect(CodeHighlight.detect('{not json')).toBeNull();
    expect(CodeHighlight.detect('')).toBeNull();
  });
});

describe('render', () => {
  it('builds text nodes and spans only: hostile page content stays text', () => {
    const hostile = '<img src=x onerror="alert(1)"></script><script>alert(2)</script>"<b>"';
    for (const lang of CodeHighlight.LANGS) {
      expect(CodeHighlight.render(out(), hostile, lang)).toBe('on');
      expect(out().textContent).toBe(hostile);
      expect(out().querySelectorAll(':not(span)')).toHaveLength(0);
      expect(out().querySelector('img, script, b')).toBeNull();
      expect([...out().querySelectorAll('span')].every((span) => span.attributes.length === 1 && /^hl-[a-z]+$/.test(span.className))).toBe(true);
    }
    const json = JSON.stringify({ html: '</script><img onerror=alert(1)>' });
    CodeHighlight.render(out(), json, 'json');
    expect(out().textContent).toBe(json);
    expect(out().querySelector('img')).toBeNull();
  });

  it('colours each token kind with a class and leaves plain text as text', () => {
    expect(CodeHighlight.render(out(), '{"a": 1}', 'json')).toBe('on');
    expect(out().dataset.highlight).toBe('on');
    // Punctuation is no span of its own: it joins the text around it (review of RB-080-65).
    expect([...out().querySelectorAll('span')].map((span) => span.className)).toEqual(['hl-key', 'hl-number']);
    expect([...out().childNodes].map((node) => node.textContent)).toEqual(['{', '"a"', ': ', '1', '}']);
    expect(document.head.querySelectorAll('style[data-highlight-style]')).toHaveLength(1);
    CodeHighlight.render(out(), '[]', 'json');
    expect(document.head.querySelectorAll('style[data-highlight-style]')).toHaveLength(1);
  });

  it('shows one colour when ui.syntaxHighlight is off, and without a language', () => {
    expect(CodeHighlight.configure({ 'ui.syntaxHighlight': false })).toBe(false);
    expect(CodeHighlight.enabled).toBe(false);
    expect(CodeHighlight.render(out(), '{"a": 1}', 'json')).toBe('off');
    expect(out().children).toHaveLength(0);
    expect(out().textContent).toBe('{"a": 1}');
    expect(CodeHighlight.configure({ 'ui.syntaxHighlight': true })).toBe(true);
    expect(CodeHighlight.render(out(), 'Hello', null)).toBe('plain');
    expect(out().dataset.highlight).toBe('plain');
    // Anything but `false` keeps the default: on.
    expect(CodeHighlight.configure({ 'ui.syntaxHighlight': 'no' })).toBe(true);
    expect(CodeHighlight.configure(null)).toBe(true);
  });

  it('leaves more than 200 KiB plain, with the note the panel gives', () => {
    const big = '"' + 'x'.repeat(CodeHighlight.LIMIT_BYTES) + '"';
    expect(CodeHighlight.LIMIT_BYTES).toBe(200 * 1024);
    expect(CodeHighlight.tooLarge(big)).toBe(true);
    expect(CodeHighlight.tooLarge('é'.repeat(CodeHighlight.LIMIT_BYTES / 2 + 1))).toBe(true);
    expect(CodeHighlight.tooLarge('x'.repeat(CodeHighlight.LIMIT_BYTES))).toBe(false);
    expect(CodeHighlight.render(out(), big, 'json', 'Too large to colour')).toBe('large');
    expect(out().children).toHaveLength(0);
    expect(out().textContent).toBe(big);
    expect(out().dataset.highlightNote).toBe('Too large to colour');
    expect(out().title).toBe('Too large to colour');
    CodeHighlight.render(out(), '1', 'json');
    expect(out().dataset.highlightNote).toBeUndefined();
    expect(out().title).toBe('');
  });

  it('draws its colours from theme variables, light and dark, overridable by the panel', () => {
    CodeHighlight.render(out(), '1', 'json');
    const css = document.head.querySelector('style[data-highlight-style]')?.textContent ?? '';
    expect(css).toContain(':where(:root)');
    expect(css).toContain('@media (prefers-color-scheme: dark)');
    expect(css).toContain('.hl-key { color: var(--hl-key); }');
  });
});

describe('the colours', () => {
  function luminance(hex: string): number {
    const channels = [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255);
    const [r, g, b] = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
  }
  const contrast = (a: string, b: string): number => {
    const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
    return ((hi ?? 0) + 0.05) / ((lo ?? 0) + 0.05);
  };
  /** The panels' backgrounds and the row and cell tints code shows on. */
  const BACKGROUNDS = {
    light: ['#ffffff', '#fce8e6', '#fef7e0', '#f3e8fd', '#e8f0fe', '#f1f3f4'],
    dark: ['#1f1f1f', '#3c1f1e', '#3a3000', '#2d1f3a', '#1e2a3d', '#2a2b2e'],
  };

  it.each(['light', 'dark'] as const)('meet WCAG AA (4.5:1) on every %s background', (scheme) => {
    CodeHighlight.render(out(), '1', 'json');
    const css = document.head.querySelector('style[data-highlight-style]')?.textContent ?? '';
    const start = scheme === 'light' ? css.indexOf(':where(:root)') : css.indexOf('@media (prefers-color-scheme: dark)');
    const block = css.slice(start, css.indexOf('}', start));
    const colours = [...block.matchAll(/--hl-([\w-]+):\s*(#[0-9a-f]{6})/gi)];
    expect(colours.length).toBeGreaterThanOrEqual(9);
    for (const [, name, colour] of colours) {
      for (const bg of BACKGROUNDS[scheme]) {
        expect(contrast(colour ?? '', bg), `--hl-${name} ${colour} on ${bg}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});

describe('the editor', () => {
  it('colours a textarea over its transparent text, follows its value and input, and stays one field', async () => {
    document.body.innerHTML = '<label for="code">Code</label><div id="box"><textarea id="code"></textarea></div>';
    const area = document.getElementById('code') as HTMLTextAreaElement;
    CodeHighlight.editor(area, 'js');
    const wrap = area.parentElement as HTMLElement;
    expect(wrap.className).toBe('hl-editor');
    expect(wrap.parentElement?.id).toBe('box');
    const backdrop = wrap.querySelector('.hl-backdrop') as HTMLElement;
    expect(backdrop.getAttribute('aria-hidden')).toBe('true');
    area.value = 'return 1';
    expect(area.value).toBe('return 1');
    expect(backdrop.querySelector('.hl-keyword')?.textContent).toBe('return');
    area.value = '<img onerror=alert(1)>';
    area.dispatchEvent(new Event('input'));
    await frame();
    expect(backdrop.querySelector('img')).toBeNull();
    expect(backdrop.textContent).toBe('<img onerror=alert(1)>\n');
    expect(wrap.dataset.highlight).toBe('on');
  });

  it('chooses the language from the text, and switches off with the setting', () => {
    document.body.innerHTML = '<textarea id="value"></textarea>';
    const area = document.getElementById('value') as HTMLTextAreaElement;
    CodeHighlight.editor(area, (text) => CodeHighlight.detect(text));
    const wrap = area.parentElement as HTMLElement;
    area.value = '{"a": 1}';
    expect(wrap.querySelector('.hl-key')).not.toBeNull();
    area.value = 'plain words';
    expect(wrap.dataset.highlight).toBe('plain');
    area.value = '{"a": 1}';
    CodeHighlight.configure({ 'ui.syntaxHighlight': false });
    expect(wrap.dataset.highlight).toBe('off');
    expect(wrap.querySelector('.hl-key')).toBeNull();
    CodeHighlight.configure({ 'ui.syntaxHighlight': true });
    expect(wrap.dataset.highlight).toBe('on');
  });

  it('says above a field too large to colour why it is one colour', () => {
    document.body.innerHTML = '<textarea id="big"></textarea>';
    const area = document.getElementById('big') as HTMLTextAreaElement;
    CodeHighlight.editor(area, 'js', () => 'Too large to colour');
    const wrap = area.parentElement as HTMLElement;
    area.value = 'x'.repeat(CodeHighlight.LIMIT_BYTES + 1);
    expect(wrap.dataset.highlight).toBe('large');
    expect(wrap.dataset.highlightNote).toBe('Too large to colour');
    area.value = 'x';
    expect(wrap.dataset.highlightNote).toBeUndefined();
  });
});

/** One animation frame: what the editor batched has been painted. */
const frame = (): Promise<void> => new Promise((resolve) => requestAnimationFrame(() => resolve()));

/** The highlighter again, with a count of the tokenizer's calls. */
function counted(): { calls: () => number } {
  document.head.innerHTML = '';
  runClassicScripts([
    sources[0] ?? '',
    'const tokenize = CodeTokens.tokenize; globalThis.tokenized = 0;' +
      'globalThis.CodeTokens = { ...CodeTokens, tokenize: (text, lang) => { globalThis.tokenized += 1; return tokenize(text, lang); } };',
    sources[1] ?? '',
  ]);
  return { calls: () => (globalThis as unknown as { tokenized: number }).tokenized };
}

// Review of RB-080-65: the cost of polling and typing.
describe('the cost', () => {
  it('a text shown again on the next poll is cloned, not tokenized again', () => {
    const { calls } = counted();
    CodeHighlight.render(out(), '{"a": [1, 2]}', 'json');
    CodeHighlight.render(out(), '{"a": [1, 2]}', 'json');
    expect(calls()).toBe(1);
    expect(out().querySelector('.hl-key')?.textContent).toBe('"a"');
    CodeHighlight.render(out(), '{"b": 3}', 'json');
    expect(calls()).toBe(2);
  });

  it('typing colours once per frame, and asks for the language only when its kind can change', async () => {
    const { calls } = counted();
    document.body.innerHTML = '<textarea id="code"></textarea>';
    const area = document.getElementById('code') as HTMLTextAreaElement;
    let asked = 0;
    CodeHighlight.editor(area, (text) => {
      asked += 1;
      return CodeHighlight.detect(text);
    });
    area.value = '{"a": 1}';
    const before = calls();
    const askedBefore = asked;
    for (const next of ['{"a": 12}', '{"a": 123}', '{"a": 1234}']) {
      valueSetter(area, next);
      area.dispatchEvent(new Event('input'));
    }
    expect(calls()).toBe(before);
    await frame();
    expect(calls()).toBe(before + 1);
    expect(asked).toBe(askedBefore);
    expect(area.parentElement?.querySelector('.hl-number')?.textContent).toBe('1234');
  });

  it('switched off or too large, the editor builds no coloured copy at all', () => {
    const { calls } = counted();
    document.body.innerHTML = '<textarea id="code"></textarea>';
    const area = document.getElementById('code') as HTMLTextAreaElement;
    CodeHighlight.editor(area, 'js');
    const backdrop = area.parentElement?.querySelector('.hl-backdrop') as HTMLElement;
    area.value = 'x'.repeat(CodeHighlight.LIMIT_BYTES + 1);
    expect(backdrop.childNodes).toHaveLength(0);
    CodeHighlight.configure({ 'ui.syntaxHighlight': false });
    area.value = 'return 1';
    expect(backdrop.childNodes).toHaveLength(0);
    expect(calls()).toBe(0);
  });
});

/** Sets a textarea's value as typing does: past the editor's own `value` setter. */
function valueSetter(area: HTMLTextAreaElement, text: string): void {
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(area, text);
}

describe('the editor follows its textarea', () => {
  it('in size: the coloured copy is as large as the field, which resizes only vertically', () => {
    document.body.innerHTML = '<textarea id="code"></textarea>';
    const area = document.getElementById('code') as HTMLTextAreaElement;
    Object.defineProperty(area, 'offsetWidth', { value: 300 });
    Object.defineProperty(area, 'offsetHeight', { value: 120 });
    CodeHighlight.editor(area, 'js');
    const backdrop = area.parentElement?.querySelector('.hl-backdrop') as HTMLElement;
    expect(backdrop.style.width).toBe('300px');
    expect(backdrop.style.height).toBe('120px');
    const css = document.head.querySelector('style[data-highlight-style]')?.textContent ?? '';
    expect(css).toContain('.hl-editor > textarea { display: block; width: 100%; box-sizing: border-box; resize: vertical; }');
  });

  it('in text set any way: setRangeText, defaultValue, a form reset, focus', async () => {
    document.body.innerHTML = '<form id="form"><textarea id="code"></textarea></form>';
    const area = document.getElementById('code') as HTMLTextAreaElement;
    CodeHighlight.editor(area, 'js');
    const backdrop = area.parentElement?.querySelector('.hl-backdrop') as HTMLElement;
    area.setRangeText('return 1');
    expect(backdrop.querySelector('.hl-keyword')?.textContent).toBe('return');
    area.defaultValue = 'const a = 1';
    area.value = 'let b';
    (document.getElementById('form') as HTMLFormElement).reset();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(backdrop.textContent).toBe(area.value + '\n');
    valueSetter(area, 'throw x');
    area.dispatchEvent(new Event('focus'));
    expect(backdrop.querySelector('.hl-keyword')?.textContent).toBe('throw');
  });
});

describe('values a page answered', () => {
  it('are text when they are strings, JavaScript when functions, JSON when JSON', () => {
    expect(CodeHighlight.valueLang('string', '"don\'t do this"')).toBeNull();
    expect(CodeHighlight.valueLang('object', '{"a":1}')).toBe('json');
    expect(CodeHighlight.valueLang('number', '42')).toBe('json');
    expect(CodeHighlight.valueLang('function', 'function f()')).toBe('js');
    expect(CodeHighlight.valueLang('object', '<div#main>')).toBeNull();
    expect(CodeHighlight.valueLang('undefined', 'undefined')).toBeNull();
  });
});

describe('the panels that load it', () => {
  /** The names a plugin's web-common.txt lists. */
  const listed = (plugin: string): string[] =>
    readFileSync(join(PLUGINS, plugin, 'web-common.txt'), 'utf8')
      .split('\n')
      .map((line) => (line.split('#')[0] ?? '').trim())
      .filter(Boolean);

  // Review of RB-080-65: one source, no checked-in copies. Since RB-080-84 the panels bundle it
  // (plugins/web-common/panel-ui/src/code) into their built panel/dist/panel.js.
  it.each(HIGHLIGHT_PANELS)('plugins/%s bundles it from here and ships no copy', (plugin) => {
    for (const file of FILES) {
      expect(existsSync(join(PLUGINS, plugin, 'panel', file)), `plugins/${plugin}/panel/${file}: delete the copy`).toBe(false);
    }
    expect(existsSync(join(PLUGINS, plugin, 'web-common.txt')), `plugins/${plugin}/web-common.txt: no staging`).toBe(false);
    expect(readFileSync(join(PLUGINS, plugin, 'panel', 'dist', 'panel.js'), 'utf8')).toContain('ui.syntaxHighlight');
  });

  it('every file a web-common.txt lists exists, and every file a panel page loads is its own or listed', () => {
    const plugins = readdirSync(PLUGINS).filter((id) => existsSync(join(PLUGINS, id, 'web-common.txt')));
    for (const plugin of plugins) {
      const names = listed(plugin);
      for (const name of names) {
        expect(existsSync(join(PLUGINS, 'web-common', 'panel', name)), `${plugin}: ${name}`).toBe(true);
      }
      const page = readFileSync(join(PLUGINS, plugin, 'panel', 'index.html'), 'utf8');
      const loaded = [...page.matchAll(/<(?:script src|link rel="stylesheet" href)="([^"]+)"/g)].map((match) => match[1] ?? '');
      for (const file of loaded) {
        const own = existsSync(join(PLUGINS, plugin, 'panel', file));
        expect(own || names.includes(file), `plugins/${plugin}/panel/index.html loads ${file}`).toBe(true);
      }
    }
  });
});
