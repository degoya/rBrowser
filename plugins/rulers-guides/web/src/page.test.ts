import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { drawing, fakePlugin, runPageScript, settle, type FakePlugin, type Message } from './support';

let plugin: FakePlugin;

function host(): HTMLElement {
  const found = document.querySelector('[data-rbrowser-plugin="rulers-guides"]');
  if (!(found instanceof HTMLElement)) throw new Error('no overlay host');
  return found;
}

function answer(layout: Record<string, unknown>) {
  return (message: Message) =>
    message.op === 'init'
      ? { state: {}, overrides: { visible: {} }, prefs: { unit: 'px', snapPx: 8 }, layout }
      : message.op === 'layout'
        ? { layout }
        : { state: {}, saved: true };
}

async function start(layout = drawing()): Promise<ShadowRoot> {
  plugin = fakePlugin(answer(layout));
  const root = runPageScript(plugin);
  await settle();
  return root;
}

function pointer(type: string, init: PointerEventInit): PointerEvent {
  return new PointerEvent(type, { bubbles: true, composed: true, button: 0, pointerId: 1, ...init });
}

beforeEach(() => {
  document.documentElement.querySelectorAll('[data-rbrowser-plugin]').forEach((node) => node.remove());
});

afterEach(() => {
  plugin?.stop();
});

describe('the page script', () => {
  it('asks for its drawing with the page widths', async () => {
    await start();
    expect(plugin.sent[0]).toMatchObject({ op: 'init', overrides: null });
    expect(plugin.sent[0]?.geometry).toEqual({
      clientWidth: document.documentElement.clientWidth,
      innerWidth: window.innerWidth,
    });
  });

  it('draws in a fixed host that takes no pointer events and cannot widen the page', async () => {
    await start();
    const style = host().style;
    expect(style.position).toBe('fixed');
    expect(style.pointerEvents).toBe('none');
    expect(style.overflow).toBe('hidden');
    expect(style.display).toBe('block');
    expect(host().getAttribute('aria-hidden')).toBe('true');
  });

  it("is the custom element rbrowser-rulers-guides, so the sync agent ignores what happens in it", async () => {
    await start();
    // agent/src/capture.ts passes over every event whose path crosses an `rbrowser-*` element:
    // dragging a guide in one view is never replayed in the others.
    expect(host().localName).toBe('rbrowser-rulers-guides');
  });

  it("lives in rBrowser's one host of page overlays and takes it away when stopped (RB-090-51)", async () => {
    await start();
    const layers = host().parentElement;
    expect(layers?.localName).toBe('rbrowser-layers');
    expect(layers?.parentElement).toBe(document.documentElement);
    plugin.stop();
    expect(document.querySelector('[data-rbrowser-plugin="rulers-guides"]')).toBeNull();
    expect(layers?.isConnected).toBe(false);
  });

  it('draws the guides at their page px', async () => {
    const root = await start();
    const vertical = root.querySelector<HTMLElement>('.guides-x .guide.x');
    const horizontal = root.querySelector<HTMLElement>('.guides-y .guide.y');
    expect(vertical?.style.left).toBe('320px');
    expect(horizontal?.style.top).toBe('200px');
    // Their handles sit in the ruler that measures them.
    expect(root.querySelector<HTMLElement>('.top .mark')?.dataset.id).toBe('1');
    expect(root.querySelector<HTMLElement>('.left .mark')?.dataset.id).toBe('2');
  });

  it('draws the columns and the baseline grid', async () => {
    const root = await start();
    const columns = [...root.querySelectorAll<HTMLElement>('.columns div')];
    expect(columns.map((column) => [column.style.left, column.style.width])).toEqual([
      ['16px', '73.75px'],
      ['105.75px', '73.75px'],
      ['195.5px', '73.75px'],
      ['285.25px', '73.75px'],
    ]);
    const baseline = root.querySelector<HTMLElement>('.baseline');
    expect(baseline?.style.backgroundSize).toBe('100% 8px');
    expect(baseline?.hidden).toBe(false);
  });

  it('labels the rulers every 100 page px from the scroll position', async () => {
    const root = await start();
    const labels = [...root.querySelectorAll<HTMLElement>('.top .label')];
    expect(labels[0]?.textContent).toBe('0');
    const at200 = labels.find((label) => label.dataset.value === '200');
    expect(at200?.style.left).toBe('200px');
    expect(root.querySelector<HTMLElement>('.left .label[data-value="100"]')?.style.top).toBe('100px');
  });

  it('hides what is switched off and the whole host when nothing is shown', async () => {
    const root = await start(drawing({ visible: { rulers: false, columns: true, baseline: false } }));
    expect(root.querySelector<HTMLElement>('.ruler.top')?.hidden).toBe(true);
    expect(root.querySelector<HTMLElement>('.guides-x')?.hidden).toBe(true);
    expect(root.querySelector<HTMLElement>('.columns')?.hidden).toBe(false);
    expect(root.querySelector<HTMLElement>('.baseline')?.hidden).toBe(true);
    plugin.stop();
    await start(drawing({ visible: { rulers: false, columns: false, baseline: false } }));
    expect(host().style.display).toBe('none');
  });

  it('asks again with the posted overrides when another view changes the state', async () => {
    await start();
    plugin.post({ state: {}, overrides: { guides: [] } });
    await settle();
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'layout', overrides: { guides: [] } });
    // Anything else posted is ignored.
    const count = plugin.sent.length;
    plugin.post('hello');
    plugin.post({ overrides: 'no object' });
    await settle();
    expect(plugin.sent.length).toBe(count);
  });

  it('drags a new horizontal guide out of the top ruler', async () => {
    const root = await start();
    const top = root.querySelector<HTMLElement>('.ruler.top');
    top?.dispatchEvent(pointer('pointerdown', { clientX: 100, clientY: 8 }));
    expect(root.querySelector('.guide.preview')).not.toBeNull();
    window.dispatchEvent(pointer('pointermove', { clientX: 100, clientY: 203 }));
    window.dispatchEvent(pointer('pointerup', { clientX: 100, clientY: 203, shiftKey: true }));
    await settle();
    expect(root.querySelector('.guide.preview')).toBeNull();
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'guide', action: 'add', axis: 'y', pos: 203, snap: true });
  });

  it('drops a new guide let go over its own ruler', async () => {
    const root = await start();
    root.querySelector<HTMLElement>('.ruler.left')?.dispatchEvent(pointer('pointerdown', { clientX: 8, clientY: 100 }));
    window.dispatchEvent(pointer('pointerup', { clientX: 10, clientY: 100 }));
    await settle();
    expect(plugin.sent.at(-1)?.op).toBe('init');
  });

  it('moves a guide by its handle, removes it by double-click, cancels with Escape', async () => {
    const root = await start();
    const mark = root.querySelector<HTMLElement>('.top .mark');
    mark?.dispatchEvent(pointer('pointerdown', { clientX: 320, clientY: 10 }));
    window.dispatchEvent(pointer('pointerup', { clientX: 360, clientY: 300 }));
    await settle();
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'guide', action: 'move', id: 1, pos: 360 });

    // A click on the handle without moving it sends nothing.
    const count = plugin.sent.length;
    mark?.dispatchEvent(pointer('pointerdown', { clientX: 320, clientY: 10 }));
    window.dispatchEvent(pointer('pointerup', { clientX: 320, clientY: 10 }));
    await settle();
    expect(plugin.sent.length).toBe(count);

    mark?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    await settle();
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'guide', action: 'remove', id: 1 });

    mark?.dispatchEvent(pointer('pointerdown', { clientX: 320, clientY: 10 }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    window.dispatchEvent(pointer('pointerup', { clientX: 500, clientY: 10 }));
    await settle();
    expect(plugin.sent.at(-1)?.action).toBe('remove');
  });

  it('removes everything it added when the host stops it', async () => {
    await start();
    expect(host().isConnected).toBe(true);
    plugin.stop();
    expect(document.querySelector('[data-rbrowser-plugin]')).toBeNull();
    const count = plugin.sent.length;
    plugin.post({ overrides: {} });
    window.dispatchEvent(new Event('resize'));
    await settle();
    expect(plugin.sent.length).toBe(count);
  });

  // RB-080-66: seven views each asked again on every post and resize, while nothing was shown
  // as well, behind a component that answers one call at a time - the owner's console showed
  // `plugins.error.busy` 39 times and `plugins.error.timed_out` 25 times.
  it('keeps one request in flight: posts while a drawing is asked for make one more request', async () => {
    await start();
    const waiting: Array<(value: unknown) => void> = [];
    plugin.answer = (message) =>
      message.op === 'layout' ? new Promise((resolve) => waiting.push(resolve)) : { state: {}, saved: true };
    const before = plugin.sent.length;
    for (let index = 0; index < 10; index += 1) plugin.post({ state: { visible: { rulers: true } }, overrides: { n: index } });
    await settle();
    expect(plugin.sent.length - before).toBe(1);
    waiting.shift()?.({ layout: drawing() });
    await settle(10);
    // The one more carries the newest overrides.
    expect(plugin.sent.length - before).toBe(2);
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'layout', overrides: { n: 9 } });
    waiting.shift()?.({ layout: drawing() });
    await settle(10);
    expect(plugin.sent.length - before).toBe(2);
  });

  it('asks nothing on resize or post while nothing is shown', async () => {
    vi.useFakeTimers();
    try {
      const hidden = { rulers: false, columns: false, baseline: false };
      plugin = fakePlugin(answer(drawing({ visible: hidden })));
      runPageScript(plugin);
      await settle();
      const before = plugin.sent.length;
      // A second of resizes, one every 150 ms (the grid reflowing), and a post that keeps all off.
      for (let tick = 0; tick < 7; tick += 1) {
        window.dispatchEvent(new Event('resize'));
        await vi.advanceTimersByTimeAsync(150);
      }
      plugin.post({ state: { visible: hidden }, overrides: { guides: [] } });
      await settle();
      expect(plugin.sent.length - before).toBe(0);
      // Switched on in another view: asked at once, with the posted overrides.
      plugin.post({ state: { visible: { ...hidden, rulers: true } }, overrides: { guides: [1] } });
      await settle();
      expect(plugin.sent.length - before).toBe(1);
      expect(plugin.sent.at(-1)).toMatchObject({ op: 'layout', overrides: { guides: [1] } });
    } finally {
      vi.useRealTimers();
    }
  });

  it('asks for the drawing again after a resize while something is shown', async () => {
    vi.useFakeTimers();
    try {
      plugin = fakePlugin(answer(drawing()));
      runPageScript(plugin);
      await settle();
      const before = plugin.sent.length;
      window.dispatchEvent(new Event('resize'));
      await vi.advanceTimersByTimeAsync(150);
      expect(plugin.sent.length - before).toBe(1);
      expect(plugin.sent.at(-1)?.op).toBe('layout');
    } finally {
      vi.useRealTimers();
    }
  });

  it('starts again on the next occasion when its first request failed', async () => {
    vi.useFakeTimers();
    try {
      let fail = true;
      plugin = fakePlugin((message) => (fail ? new Error('plugins.error.busy') : answer(drawing())(message)));
      runPageScript(plugin);
      await settle();
      expect(plugin.sent.map((message) => message.op)).toEqual(['init']);
      fail = false;
      window.dispatchEvent(new Event('resize'));
      await vi.advanceTimersByTimeAsync(150);
      expect(plugin.sent.map((message) => message.op)).toEqual(['init', 'init']);
      expect(host().style.display).toBe('block');
    } finally {
      vi.useRealTimers();
    }
  });

  it("reports a failed request to rBrowser, never to the page's console", async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    try {
      plugin = fakePlugin(() => new Error('plugins.error.busy'));
      runPageScript(plugin);
      await settle();
      expect(warn).not.toHaveBeenCalled();
      expect(plugin.logged).toEqual([{ level: 'warning', values: [new Error('plugins.error.busy')] }]);
    } finally {
      warn.mockRestore();
    }
  });

  // Review of RB-080-66, 2: overrides adopted while `init` was in flight are newer than its answer's.
  it('keeps the overrides posted while its start was in flight', async () => {
    const waiting: Array<(value: unknown) => void> = [];
    plugin = fakePlugin((message) =>
      message.op === 'init' ? new Promise((resolve) => waiting.push(resolve)) : { layout: drawing() },
    );
    runPageScript(plugin);
    await settle();
    plugin.post({ state: { visible: { rulers: true } }, overrides: { n: 'new' } });
    waiting.shift()?.({ state: {}, overrides: { n: 'old' }, prefs: { unit: 'px', snapPx: 8 }, layout: drawing() });
    await settle(10);
    expect(plugin.sent.map((message) => message.op)).toEqual(['init', 'layout']);
    expect(plugin.sent.at(-1)).toMatchObject({ overrides: { n: 'new' } });
  });

  // Review of RB-080-66, 3: an answer asked before everything was switched off shows nothing again.
  it('stays hidden when an answer from before the switch-off arrives', async () => {
    await start();
    const waiting: Array<(value: unknown) => void> = [];
    plugin.answer = (message) => (message.op === 'layout' ? new Promise((resolve) => waiting.push(resolve)) : {});
    plugin.post({ state: { visible: { rulers: true } }, overrides: { n: 1 } });
    await settle();
    const hidden = { rulers: false, columns: false, baseline: false };
    plugin.post({ state: { visible: hidden }, overrides: { n: 2 } });
    expect(host().style.display).toBe('none');
    const count = plugin.sent.length;
    waiting.shift()?.({ layout: drawing() });
    await settle(10);
    expect(host().style.display).toBe('none');
    expect(plugin.sent.length).toBe(count);
  });

  it('runs in the top frame only', () => {
    // The page script's first statement; a frame's window is not its top.
    expect(window === window.top).toBe(true);
  });
});
