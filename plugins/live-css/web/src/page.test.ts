// The page script (`page/live-css.js`) against the fake bridge in happy-dom: the sheet it adopts,
// the styles that follow from it, and what it never does (markup, other frames' work). The real
// browser, its media queries per view width and a navigation are test:e2e's.
import { afterEach, describe, expect, it } from 'vitest';
import source from '../../page/live-css.js?raw';
import { type FakePlugin, type Message, fakePlugin, runPageScriptRoots, settle } from '../../../web-common/src/harness';

const OUTLINE = 'body { outline: 3px solid red }';

let plugin: FakePlugin | undefined;

/** Runs the page script; `answer` replies to its requests. */
async function start(answer: (message: Message) => unknown = () => ({ css: OUTLINE })): Promise<FakePlugin> {
  plugin = fakePlugin('live-css', answer);
  const roots = runPageScriptRoots(source, plugin);
  expect(roots).toEqual([]);
  await settle(4);
  return plugin;
}

const ours = (): CSSStyleSheet[] => [...document.adoptedStyleSheets];
const outline = (): string => getComputedStyle(document.body).outlineStyle;
const text = (sheet: CSSStyleSheet | undefined): string => [...(sheet?.cssRules ?? [])].map((rule) => rule.cssText).join('\n');

afterEach(() => {
  plugin?.stop();
  plugin = undefined;
  document.adoptedStyleSheets = [];
  document.head.innerHTML = '';
  document.body.innerHTML = '';
});

describe('the Live CSS page script', () => {
  it('asks for the sheet as the document starts and adopts it', async () => {
    const bridge = await start();
    expect(bridge.sent).toEqual([{ op: 'sheet', reason: 'start' }]);
    expect(ours()).toHaveLength(1);
    expect(text(ours()[0])).toContain('outline');
    expect(outline()).toBe('solid');
  });

  it('adopts nothing for an empty sheet and builds no markup', async () => {
    await start(() => ({ css: '' }));
    expect(ours()).toEqual([]);
    expect(document.querySelectorAll('style, link, script')).toHaveLength(0);
  });

  it('follows the posts: a new text, then off without a trace, then on again', async () => {
    const bridge = await start();
    bridge.post({ css: 'body { outline: 1px dotted blue }' });
    expect(outline()).toBe('dotted');
    bridge.post({ css: '' });
    expect(ours()).toEqual([]);
    expect(outline()).not.toBe('dotted');
    bridge.post({ css: OUTLINE });
    expect(ours()).toHaveLength(1);
    expect(outline()).toBe('solid');
  });

  it('keeps its sheet last among the adopted ones and puts it back after the page replaced them', async () => {
    const bridge = await start();
    const page = new CSSStyleSheet();
    page.replaceSync('body { outline: 2px dashed green }');
    document.adoptedStyleSheets = [...document.adoptedStyleSheets, page];
    bridge.post({ css: OUTLINE });
    expect(ours().at(-1)).not.toBe(page);
    expect(ours()).toHaveLength(2);
    expect(outline()).toBe('solid');
    document.adoptedStyleSheets = [page];
    bridge.post({ css: OUTLINE });
    expect(ours()).toHaveLength(2);
    expect(ours()[0]).toBe(page);
  });

  it('takes markup in the CSS as CSS: nothing reaches the HTML parser', async () => {
    const bridge = await start();
    bridge.post({ css: 'a { color: red } </style><script>window.hacked = true</script><style>' });
    await settle(4);
    expect(document.querySelectorAll('script, style')).toHaveLength(0);
    expect((window as unknown as { hacked?: boolean }).hacked).toBeUndefined();
  });

  it('asks again on a refresh post and drops an answer a newer post overtook', async () => {
    let release: (value: unknown) => void = () => undefined;
    const bridge = await start((message) =>
      message.reason === 'refresh' ? new Promise((resolve) => (release = resolve)) : { css: '' },
    );
    bridge.post({ refresh: true });
    expect(bridge.sent.at(-1)).toEqual({ op: 'sheet', reason: 'refresh' });
    bridge.post({ css: OUTLINE });
    release({ css: 'body { outline: 1px dotted blue }' });
    await settle(4);
    expect(outline()).toBe('solid');
  });

  it('passes over posts that are no sheet', async () => {
    const bridge = await start();
    for (const message of [null, 'css', { css: 3 }, { $settings: true }]) bridge.post(message);
    expect(outline()).toBe('solid');
    expect(bridge.sent).toHaveLength(1);
  });

  it('logs a request the host refused and adopts nothing', async () => {
    const bridge = await start(() => new Error('plugins.error.busy'));
    expect(ours()).toEqual([]);
    expect(bridge.logged).toHaveLength(1);
    expect(bridge.logged[0]?.level).toBe('warning');
  });

  it('takes its sheet out when the host stops it and hears nothing after', async () => {
    const bridge = await start();
    bridge.stop();
    expect(ours()).toEqual([]);
    bridge.post({ css: OUTLINE });
    expect(ours()).toEqual([]);
  });
});
