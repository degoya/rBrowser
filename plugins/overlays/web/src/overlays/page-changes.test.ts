import { afterEach, describe, expect, it, vi } from 'vitest';
import { Controller } from '../controller';
import type { State } from '../types';
import { chaosText, LONG_WORD } from './chaos';
import { deepActive, describe as describeElement } from './focus';
import { findForced } from './force-state';
import { fullPath, scanAnswer } from './scan';
import { findSmallText } from './small-text';
import { DEFAULT_SETTINGS } from '../types';

const PAGE =
  '<style>main { color: red }</style><main><h1 style="color: blue">Title</h1><p class="note" style="font-size: 10px">Small print here</p>' +
  '<p>A paragraph of text.</p><img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" alt=""><button id="b">Go</button></main>';
const sink = () => ({ emit: vi.fn(), send: vi.fn() });
const on = (ids: string[], options: Record<string, Record<string, unknown>> = {}): State => ({
  version: 1,
  all: Object.fromEntries(ids.map((id) => [id, { enabled: true, options: options[id] ?? {} }])),
  views: {},
});

let controller: Controller | undefined;
afterEach(() => {
  controller?.destroy();
  controller = undefined;
});

describe('content chaos', () => {
  it('makes a text longer with a long word, or shorter at a word, and keeps its white space', () => {
    const longer = chaosText(' one two three ', 2);
    expect(longer.startsWith(' one ')).toBe(true);
    expect(longer.endsWith(' ')).toBe(true);
    expect(longer).toContain(LONG_WORD);
    expect(longer.trim().length).toBeGreaterThanOrEqual('one two three'.length * 2);
    expect(chaosText('one two three four', 0.5)).toBe('one two');
    expect(chaosText('   ', 2)).toBe('   ');
    expect(chaosText('same', 1)).toBe('same');
  });
});

describe('the overlays that change the page', () => {
  it('leave the DOM and the adopted sheets as they found them when switched off', () => {
    document.body.innerHTML = PAGE;
    const before = document.documentElement.outerHTML;
    const sheets = document.adoptedStyleSheets.length;
    controller = new Controller(document, window, 1, sink());
    const all = ['text-spacing', 'content-chaos', 'placeholdifier', 'css-off', 'images-off', 'focus-tracking', 'stacking', 'small-text'];
    controller.apply(on(all, { 'css-off': { includeInline: true } }), {});
    expect(document.body.textContent).toContain(LONG_WORD);
    expect(document.adoptedStyleSheets.length).toBeGreaterThan(sheets);
    expect(Array.from(document.styleSheets).every((sheet) => sheet.disabled)).toBe(true);
    controller.apply({ version: 1, all: {}, views: {} }, {});
    expect(document.documentElement.outerHTML).toBe(before);
    expect(document.adoptedStyleSheets.length).toBe(sheets);
    expect(Array.from(document.styleSheets).some((sheet) => sheet.disabled)).toBe(false);
  });

  it('give a text the page changed meanwhile no old text back', () => {
    document.body.innerHTML = '<p>alpha beta</p>';
    controller = new Controller(document, window, 1, sink());
    controller.apply(on(['content-chaos'], { 'content-chaos': { factor: 0.5 } }), {});
    const text = document.querySelector('p')?.firstChild as Text;
    expect(text.nodeValue).toBe('alpha');
    text.nodeValue = 'changed by the page';
    controller.apply({ version: 1, all: {}, views: {} }, {});
    expect(text.nodeValue).toBe('changed by the page');
  });

  it('leave a sheet the page disabled itself disabled', () => {
    document.body.innerHTML = '<style>p { color: red }</style><style>p { margin: 0 }</style><p>x</p>';
    const [first, second] = Array.from(document.styleSheets);
    if (first) first.disabled = true;
    controller = new Controller(document, window, 1, sink());
    controller.apply(on(['css-off']), {});
    expect(second?.disabled).toBe(true);
    controller.apply({ version: 1, all: {}, views: {} }, {});
    expect(first?.disabled).toBe(true);
    expect(second?.disabled).toBe(false);
  });
});

describe('the pieces of the further overlays', () => {
  it('find small text by its computed font size', () => {
    document.body.innerHTML = PAGE;
    const hits = findSmallText({ doc: document, win: window, settings: DEFAULT_SETTINGS, options: {} });
    expect(hits.map((hit) => [hit.element.className, hit.label])).toEqual([['note', '10px']]);
    expect(findSmallText({ doc: document, win: window, settings: DEFAULT_SETTINGS, options: { px: 8 } })).toEqual([]);
  });

  it('frame the element a state is forced on, labelled with the states', () => {
    document.body.innerHTML = PAGE;
    const hits = findForced({ doc: document, win: window, settings: DEFAULT_SETTINGS, options: { selector: '#b', states: ['hover', 'focus'] } });
    expect(hits.map((hit) => hit.label)).toEqual([':hover:focus']);
    expect(findForced({ doc: document, win: window, settings: DEFAULT_SETTINGS, options: { selector: 'div[' } })).toEqual([]);
  });

  it('track the focused element and name it', () => {
    document.body.innerHTML = PAGE;
    const button = document.getElementById('b') as HTMLButtonElement;
    button.focus();
    expect(deepActive(document)).toBe(button);
    expect(describeElement(button)).toBe('button#b');
    button.blur();
    expect(deepActive(document)).toBeNull();
  });

  it('answer a scan with selectors, full paths and labels', () => {
    document.body.innerHTML = PAGE;
    const note = document.querySelector('.note') as Element;
    expect(fullPath(note)).toBe('html > body > main > p:nth-of-type(1)');
    const answer = scanAnswer(window, [{ element: note, label: '10px' }]);
    expect(answer.count).toBe(1);
    expect(answer.items[0]).toMatchObject({ selector: 'p.note', path: 'html > body > main > p:nth-of-type(1)', label: '10px' });
  });
});
