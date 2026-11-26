// The screenshot editor panel and large images (RB-099-24): an image of many chunks loads, one
// over what the panel takes says so, and an image too large for a canvas shows as its preview at
// 1:scale while every annotation stays in the image's own pixels.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fetchImage, MAX_BYTES, MAX_CHUNKS } from './panel/files';
import { answer, canvas, el, fromHost, key, keyboardRect, lastCall, mountEditor, settle, strings, SHOT, type Sent } from './mounted';

/** A `load` that answers `count` chunks of one byte each, the version `hash` from `changeAt` on. */
function chunked(count: number, changeAt = Infinity) {
  const calls: Record<string, unknown>[] = [];
  const call = async (_name: string, args: Record<string, unknown>): Promise<unknown> => {
    calls.push(args);
    const offset = Number(args.offset ?? 0);
    const next = offset + 1 < count ? offset + 1 : null;
    return { image: SHOT, bytes: count, offset, next, hash: offset >= changeAt ? 'h2' : 'h1', data: btoa(String.fromCharCode(offset % 256)), width: 10, height: 10 };
  };
  return { calls, call };
}

describe('loading a large image', () => {
  it('takes more chunks than an 8 MiB image had, in order, each naming the first hash', async () => {
    const { calls, call } = chunked(300);
    const { first, blob } = await fetchImage(call, SHOT);
    expect(first.offset).toBe(0);
    expect(blob.size).toBe(300);
    expect(calls).toHaveLength(300);
    expect(calls[299]).toEqual({ image: SHOT, offset: 299, hash: 'h1' });
    expect(MAX_CHUNKS * 47 * 1024).toBeGreaterThan(MAX_BYTES);
  });

  it('refuses an image larger than the panel takes before a second chunk', async () => {
    const calls: unknown[] = [];
    const call = async (_name: string, args: Record<string, unknown>): Promise<unknown> => {
      calls.push(args);
      return { image: SHOT, bytes: MAX_BYTES + 1, offset: 0, next: 1, hash: 'h1', data: '' };
    };
    await expect(fetchImage(call, SHOT)).rejects.toThrow('plugin.screenshot-editor.panel.too_many_chunks');
    expect(calls).toHaveLength(1);
  });

  it('stops when the file changes under it', async () => {
    const { call } = chunked(10, 4);
    await expect(fetchImage(call, SHOT)).rejects.toThrow('plugin.screenshot-editor.panel.changed');
  });
});

describe('an image too large for a canvas', () => {
  let stop: () => void = () => undefined;
  const sent: Sent[] = [];

  afterEach(() => {
    stop();
    sent.length = 0;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('shows its preview at 1:scale and keeps the annotations in image pixels', async () => {
    stop = mountEditor(sent);
    fromHost({ type: 'settings', strings: strings(), settings: { 'screenshot-editor.color': '#00ff00', 'screenshot-editor.strokeWidth': 6 } });
    await settle();
    await answer(sent, 'list', { images: [SHOT], truncated: false });
    // The preview decodes to 100 x 80 (mounted.ts); the image is four times that.
    await answer(sent, 'load', { image: SHOT, bytes: 3, offset: 0, next: null, hash: 'h1', data: btoa('png'), width: 400, height: 320, scale: 4, annotations: null });
    expect([canvas().width, canvas().height]).toEqual([100, 80]);
    expect([canvas().dataset.width, canvas().dataset.height]).toEqual(['400', '320']);
    expect(el('#status').textContent).toBe(`${SHOT}: 400 x 320 px, shown at 1:4; it is saved at its full size.`);
    // The keyboard cursor starts in the middle of the image, and moves in image pixels.
    await keyboardRect();
    key(document.body, 's', { ctrlKey: true });
    await settle();
    expect(lastCall(sent, 'annotate')?.args?.annotations).toEqual([{ type: 'rect', x: 200, y: 160, w: 30, h: 20, color: '#00ff00', width: 6, fill: false }]);
  });
});
