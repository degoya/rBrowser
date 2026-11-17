import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import source from '../../page/image-overlay.js?raw';
import { base64, designBytes, fakePlugin, runPageScript, settle, state, type FakePlugin, type Message } from './support';

const CHUNK = 10;
const BYTES = designBytes(25);
const DESIGN = { path: 'designs/mobile@3x.png', mime: 'image/png', bytes: 25, hash: 'abc', width: 1179, height: 2556, scale: 3, chunks: 3 };

let plugin: FakePlugin;
let decoded: Blob[] = [];
let drawnBitmaps: unknown[] = [];

function answers(initial: Record<string, unknown>, image: Record<string, unknown> | null = DESIGN) {
  return (message: Message) => {
    if (message.op === 'init') return { state: initial, overrides: {}, image, problem: null, chunkBytes: CHUNK };
    if (message.op === 'chunk') {
      const index = Number(message.index);
      return { index, data: base64(BYTES.slice(index * CHUNK, (index + 1) * CHUNK)) };
    }
    return { state: initial };
  };
}

async function start(initial = state(), image: Record<string, unknown> | null = DESIGN): Promise<ShadowRoot[]> {
  plugin = fakePlugin(answers(initial, image));
  const roots = runPageScript(plugin);
  await settle();
  return roots;
}

function hosts(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('rbrowser-image-overlay')];
}

function canvasOf(roots: ShadowRoot[]): HTMLCanvasElement {
  const canvas = roots[0]?.querySelector('canvas');
  if (!canvas) throw new Error('no canvas');
  return canvas;
}

function handleOf(roots: ShadowRoot[]): HTMLElement {
  const handle = roots[1]?.firstElementChild;
  if (!(handle instanceof HTMLElement)) throw new Error('no handle');
  return handle;
}

beforeEach(() => {
  decoded = [];
  drawnBitmaps = [];
  vi.stubGlobal('createImageBitmap', async (blob: Blob) => {
    decoded.push(blob);
    return { width: DESIGN.width, height: DESIGN.height, close: vi.fn() };
  });
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement, kind: string) {
    return kind === 'bitmaprenderer' ? ({ transferFromImageBitmap: (bitmap: unknown) => drawnBitmaps.push(bitmap) } as never) : null;
  } as never);
});

afterEach(() => {
  plugin?.stop();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  document.querySelectorAll('rbrowser-image-overlay').forEach((node) => node.remove());
});

describe('the page script', () => {
  it('asks for its state and design with the page width and pixel ratio', async () => {
    await start();
    expect(plugin.sent[0]).toMatchObject({ op: 'init', width: window.innerWidth, dpr: window.devicePixelRatio || 1 });
  });

  it('draws in rbrowser-image-overlay hosts that take no pointer events', async () => {
    await start();
    const [picture] = hosts();
    expect(picture?.localName).toBe('rbrowser-image-overlay');
    expect(picture?.getAttribute('aria-hidden')).toBe('true');
    expect(picture?.style.position).toBe('fixed');
    expect(picture?.style.pointerEvents).toBe('none');
    expect(picture?.style.overflow).toBe('hidden');
  });

  it('fetches the design in chunks and decodes exactly its bytes as its type', async () => {
    const roots = await start();
    const chunks = plugin.sent.filter((message) => message.op === 'chunk');
    expect(chunks.map((message) => message.index)).toEqual([0, 1, 2]);
    expect(chunks[0]).toEqual({ op: 'chunk', path: DESIGN.path, hash: DESIGN.hash, index: 0 });
    expect(decoded).toHaveLength(1);
    expect(decoded[0]?.type).toBe('image/png');
    expect(new Uint8Array(await decoded[0]!.arrayBuffer())).toEqual(BYTES);
    expect(drawnBitmaps).toHaveLength(1);
    const canvas = canvasOf(roots);
    expect([canvas.width, canvas.height]).toEqual([1179, 2556]);
  });

  it('sizes an @3x export to its CSS px and applies opacity, blend and invert', async () => {
    const roots = await start(state({ opacity: 40, blend: 'multiply', invert: true }));
    const canvas = canvasOf(roots);
    expect(canvas.style.width).toBe('393px');
    expect(canvas.style.height).toBe('852px');
    const [picture] = hosts();
    expect(picture?.style.opacity).toBe('0.4');
    expect(picture?.style.mixBlendMode).toBe('multiply');
    expect(canvas.style.filter).toBe('invert(1)');
  });

  it('loads nothing while the overlay is off and shows nothing', async () => {
    await start(state({ enabled: false }));
    expect(plugin.sent.filter((message) => message.op === 'chunk')).toHaveLength(0);
    expect(hosts()).toHaveLength(0);
  });

  it('loads its design once a post switches the overlay on', async () => {
    await start(state({ enabled: false }));
    plugin.answer = answers(state());
    plugin.post({ state: state(), overrides: { enabled: true }, reload: true });
    await settle();
    expect(plugin.sent.filter((message) => message.op === 'chunk')).toHaveLength(3);
    expect(plugin.sent.at(-4)).toMatchObject({ op: 'init', overrides: { enabled: true } });
    expect(hosts()).toHaveLength(1);
  });

  it('applies a posted change without loading again', async () => {
    const roots = await start();
    const sent = plugin.sent.length;
    plugin.post({ state: state({ opacity: 80, blend: 'screen', offset: { x: 5, y: -3 } }), reload: false });
    expect(plugin.sent.length).toBe(sent);
    expect(hosts()[0]?.style.opacity).toBe('0.8');
    expect(hosts()[0]?.style.mixBlendMode).toBe('screen');
    expect(canvasOf(roots).style.transform).toBe('translate(5px, -3px)');
  });

  it('pins the design to the page or fixes it in the view', async () => {
    const roots = await start(state({ offset: { x: 10, y: 20 } }));
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 300 });
    plugin.post({ state: state({ offset: { x: 10, y: 20 } }), reload: false });
    expect(canvasOf(roots).style.transform).toBe('translate(10px, -280px)');
    plugin.post({ state: state({ offset: { x: 10, y: 20 }, anchor: 'viewport' }), reload: false });
    expect(canvasOf(roots).style.transform).toBe('translate(10px, 20px)');
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 0 });
  });

  it('clips the design at the split and shows the handle there', async () => {
    const roots = await start(state({ split: { on: true, position: 30 } }));
    const layer = roots[0]?.firstElementChild as HTMLElement;
    expect(layer.style.clipPath).toBe('inset(0 70% 0 0)');
    expect(hosts()).toHaveLength(2);
    expect(handleOf(roots).style.left).toBe('30%');
    expect(handleOf(roots).style.pointerEvents).toBe('auto');
  });

  it('sends the split position the handle is let go at', async () => {
    const roots = await start(state({ split: { on: true, position: 50 } }));
    const width = window.innerWidth;
    const pointer = (type: string, clientX: number) =>
      new PointerEvent(type, { bubbles: true, composed: true, button: 0, pointerId: 1, clientX });
    handleOf(roots).dispatchEvent(pointer('pointerdown', width / 2));
    window.dispatchEvent(pointer('pointermove', width * 0.3));
    expect(handleOf(roots).style.left).toBe('30%');
    window.dispatchEvent(pointer('pointerup', width * 0.3));
    expect(plugin.sent.at(-1)).toMatchObject({ op: 'split', position: 30 });
  });

  it('shows nothing for a refused design and says why in the console', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    plugin = fakePlugin(() => ({
      state: state(),
      overrides: {},
      image: null,
      problem: { code: 'plugin.image-overlay.errors.path_outside', params: { path: '../x.png' } },
      chunkBytes: CHUNK,
    }));
    runPageScript(plugin);
    await settle();
    expect(hosts()).toHaveLength(0);
    expect(warn).toHaveBeenCalledWith('[image-overlay]', 'plugin.image-overlay.errors.path_outside');
  });

  it('starts again when the file changed during the transfer', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    let failures = 1;
    plugin = fakePlugin((message) => {
      if (message.op === 'chunk' && message.index === 1 && failures-- > 0) return new Error('plugin.image-overlay.errors.changed');
      return answers(state())(message);
    });
    runPageScript(plugin);
    await settle();
    expect(plugin.sent.filter((message) => message.op === 'init')).toHaveLength(2);
    expect(decoded).toHaveLength(1);
  });

  it('removes everything when the host stops it', async () => {
    await start(state({ split: { on: true, position: 50 } }));
    expect(hosts()).toHaveLength(2);
    plugin.stop();
    expect(hosts()).toHaveLength(0);
  });

  it('passes the design on without a URL or parsed markup', () => {
    expect(source).not.toMatch(/innerHTML|outerHTML|insertAdjacentHTML|createObjectURL|data:image|\.src\s*=/);
  });
});
