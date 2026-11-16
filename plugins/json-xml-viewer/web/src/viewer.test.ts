import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FILTER_DELAY, HOST_TAG, utf8Length, Viewer } from './viewer';

const USER = {
  id: 7,
  name: 'Ada',
  items: [{ id: 1 }, { id: 2 }, { id: 3 }],
};

function fakeBridge(config: Record<string, unknown> = {}) {
  const listeners: ((message: unknown) => void)[] = [];
  const stops: (() => void)[] = [];
  return {
    sent: [] as unknown[],
    send(message: unknown) {
      this.sent.push(message);
      return Promise.resolve({ config });
    },
    on(listener: (message: unknown) => void) {
      listeners.push(listener);
      return () => listeners.splice(listeners.indexOf(listener), 1);
    },
    onStop(listener: () => void) {
      stops.push(listener);
      return () => undefined;
    },
    post(message: unknown) {
      listeners.slice().forEach((listener) => listener(message));
    },
    stop() {
      stops.forEach((listener) => listener());
    },
  };
}

function showJson(text: string): void {
  document.body.replaceChildren();
  const pre = document.createElement('pre');
  pre.textContent = text;
  document.body.append(pre);
}

async function open(text: string, config: Record<string, unknown> = {}, firefox = false) {
  showJson(text);
  const bridge = fakeBridge(config);
  const viewer = new Viewer(window, bridge, { contentType: 'application/json', firefox });
  await viewer.start();
  return { viewer, bridge };
}

function rows(viewer: Viewer): string[] {
  return [...(viewer.shadow?.querySelectorAll('.row:not([hidden])') ?? [])].map((row) => row.textContent ?? '');
}

describe('the viewer', () => {
  beforeEach(() => {
    document.documentElement.style.removeProperty('overflow');
    document.body.style.removeProperty('display');
  });
  afterEach(() => {
    document.querySelectorAll(HOST_TAG).forEach((host) => host.remove());
  });

  it('shows a JSON document as a tree in a shadow root of its own host', async () => {
    const { viewer, bridge } = await open(JSON.stringify(USER));
    expect(bridge.sent).toEqual([{ type: 'config' }]);
    expect(document.querySelectorAll(HOST_TAG).length).toBe(1);
    expect(document.querySelector(HOST_TAG)?.shadowRoot).toBeNull();
    expect(viewer.currentMode).toBe('tree');
    expect(rows(viewer)).toEqual([
      '{',
      '"id": 7',
      '"name": "Ada"',
      '"items": [',
      '0: {…}  1 key',
      '1: {…}  1 key',
      '2: {…}  1 key',
    ]);
    expect(viewer.shadow?.querySelector('.status')?.textContent).toBe('$');
    expect(document.documentElement.style.getPropertyValue('overflow')).toBe('hidden');
    expect(document.body.style.getPropertyValue('display')).toBe('none');
    expect(document.body.querySelector('pre')?.textContent).toBe(JSON.stringify(USER));
  });

  it('names the tree and its rows for assistive technology', async () => {
    const { viewer } = await open(JSON.stringify(USER));
    const tree = viewer.shadow?.querySelector('[role="tree"]');
    expect(tree?.getAttribute('aria-activedescendant')).toBe('jxv-row-0');
    const second = viewer.shadow?.querySelector('#jxv-row-3');
    expect(second?.getAttribute('role')).toBe('treeitem');
    expect(second?.getAttribute('aria-level')).toBe('2');
    expect(second?.getAttribute('aria-expanded')).toBe('true');
    expect(second?.getAttribute('aria-posinset')).toBe('3');
    expect(second?.getAttribute('aria-setsize')).toBe('3');
  });

  it('moves and opens with the keyboard', async () => {
    const { viewer } = await open(JSON.stringify(USER));
    const tree = viewer.shadow?.querySelector('[role="tree"]') as HTMLElement;
    const key = (name: string) => tree.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true }));
    key('End');
    expect(viewer.shadow?.querySelector('.status')?.textContent).toBe('$.items[2]');
    key('ArrowRight');
    expect(rows(viewer).at(-1)).toBe('"id": 3');
    key('ArrowLeft');
    expect(rows(viewer).at(-1)).toBe('2: {…}  1 key');
    key('ArrowLeft');
    expect(viewer.shadow?.querySelector('.status')?.textContent).toBe('$.items');
    key('Home');
    expect(viewer.shadow?.querySelector('.status')?.textContent).toBe('$');
  });

  it('switches to the raw text and back on the command', async () => {
    const text = JSON.stringify(USER);
    const { viewer, bridge } = await open(text);
    bridge.post({ type: 'toggleRaw' });
    expect(viewer.currentMode).toBe('raw');
    const raw = viewer.shadow?.querySelector('.raw') as HTMLElement;
    expect(raw.hidden).toBe(false);
    expect(raw.textContent).toBe(text);
    bridge.post({ type: 'toggleRaw' });
    expect(viewer.currentMode).toBe('tree');
    expect(raw.hidden).toBe(true);
    viewer.setMode('pretty');
    expect(raw.textContent).toBe(JSON.stringify(USER, null, 2));
  });

  it('filters with JSONPath and copies the path', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    const { viewer } = await open(JSON.stringify(USER));
    const filter = viewer.shadow?.querySelector('input') as HTMLInputElement;
    filter.value = '$.items[*].id';
    viewer.runFilter();
    expect(rows(viewer)).toEqual(['$.items[0].id1', '$.items[1].id2', '$.items[2].id3']);
    expect(viewer.shadow?.querySelector('.notice')?.textContent).toBe('3 results');
    const copyPath = [...(viewer.shadow?.querySelectorAll('button') ?? [])].find(
      (button) => button.textContent === 'Copy path',
    ) as HTMLButtonElement;
    copyPath.click();
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith('$.items[0].id'));
    filter.value = '';
    viewer.runFilter();
    expect(rows(viewer)[0]).toBe('{');
  });

  it('runs the filter after typing stops', async () => {
    vi.useFakeTimers();
    try {
      const { viewer } = await open(JSON.stringify(USER));
      const filter = viewer.shadow?.querySelector('input') as HTMLInputElement;
      filter.value = 'ada';
      filter.dispatchEvent(new Event('input'));
      expect(rows(viewer)[0]).toBe('{');
      vi.advanceTimersByTime(FILTER_DELAY);
      expect(rows(viewer)).toEqual(['$.name"Ada"']);
    } finally {
      vi.useRealTimers();
    }
  });

  it('shows invalid JSON as raw text with the line and column of the error', async () => {
    const { viewer } = await open('{\n  "a": 1,\n}');
    expect(viewer.currentMode).toBe('raw');
    expect(viewer.shadow?.querySelector('.error')?.textContent).toBe(
      'Invalid JSON at line 3, column 1: unexpected character',
    );
    expect(viewer.shadow?.querySelector('mark')?.textContent).toBe('}');
    viewer.toggleRaw();
    expect(viewer.currentMode).toBe('raw');
  });

  it('renders only the rows in view of a large document', async () => {
    const big = Array.from({ length: 100000 }, (_, i) => ({ i }));
    const { viewer } = await open(JSON.stringify(big), { expandDepth: 1 });
    expect(viewer.tree?.length).toBe(100001);
    expect(rows(viewer).length).toBeLessThan(60);
    const tree = viewer.shadow?.querySelector('[role="tree"]') as HTMLElement;
    tree.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
    expect(rows(viewer)).toContain('99999: {…}  1 key');
  });

  it('keeps a document over the size limit raw, with a notice', async () => {
    const { viewer } = await open(JSON.stringify({ text: 'x'.repeat(70000) }), { maxBytes: 65536 });
    expect(viewer.tree).toBeNull();
    expect(viewer.shadow?.querySelector('[role="status"]')?.textContent).toContain('more than the viewer');
    expect(document.documentElement.style.getPropertyValue('overflow')).toBe('');
  });

  it('stays away when switched off, in Firefox, and from other documents', async () => {
    await open(JSON.stringify(USER), { enabled: false });
    expect(document.querySelector(HOST_TAG)).toBeNull();
    await open(JSON.stringify(USER), {}, true);
    expect(document.querySelector(HOST_TAG)).toBeNull();
    // The removed key `inFirefox` no longer switches it on: Firefox keeps its own viewer.
    const { viewer } = await open(JSON.stringify(USER), { inFirefox: true }, true);
    expect(viewer.shadow).toBeNull();
    viewer.stop();
    showJson('{}');
    const bridge = fakeBridge();
    await new Viewer(window, bridge, { contentType: 'text/html' }).start();
    expect(bridge.sent).toEqual([]);
  });

  it('removes itself when the host stops the script', async () => {
    const { bridge } = await open(JSON.stringify(USER));
    bridge.stop();
    expect(document.querySelector(HOST_TAG)).toBeNull();
    expect(document.documentElement.style.getPropertyValue('overflow')).toBe('');
    expect(document.body.style.getPropertyValue('display')).toBe('');
  });
});

describe('utf8Length', () => {
  it('counts UTF-8 bytes', () => {
    expect(utf8Length('a')).toBe(1);
    expect(utf8Length('\u00e9')).toBe(2);
    expect(utf8Length('\u20ac')).toBe(3);
    expect(utf8Length('\u{1F600}')).toBe(4);
  });
});
