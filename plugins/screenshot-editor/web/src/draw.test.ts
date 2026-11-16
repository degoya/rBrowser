import { describe, expect, it } from 'vitest';
import * as draw from './panel/draw';
import type { ShotAnnotation, ShotPixels } from './panel/types';
import { fakeContext, pixels, type Call } from './support';

/** The black-and-white "secret" of the component's tests (src/redact_tests.rs). */
const secret = (x: number, y: number) => ((x + 2 * y) % 3 === 0 || (x * y) % 7 === 1 ? [0, 0, 0, 255] : [255, 255, 255, 255]);

const at = (image: ShotPixels, x: number, y: number) => [...image.data.subarray((y * image.width + x) * 4, (y * image.width + x) * 4 + 4)];

describe('the redactions', () => {
  it('clip a region to the image and round it outward, as the component does', () => {
    expect(draw.clip(-5, 2.5, 20, 3.2, 10, 10)).toEqual({ left: 0, top: 2, right: 10, bottom: 6 });
    expect(draw.clip(12, 0, 5, 5, 10, 10)).toBeNull();
    expect(draw.clip(2, 2, 0, 5, 10, 10)).toBeNull();
  });

  it('pixelate into the rounded block averages, the original strokes gone', () => {
    const image = pixels(32, 24, secret);
    draw.pixelate(image, 8);
    for (let top = 0; top < 24; top += 8) {
      for (let left = 0; left < 32; left += 8) {
        let sum = 0;
        for (let y = top; y < top + 8; y += 1) for (let x = left; x < left + 8; x += 1) sum += secret(x, y)[0] ?? 0;
        const expected = Math.floor((sum + 32) / 64);
        for (let y = top; y < top + 8; y += 1) {
          for (let x = left; x < left + 8; x += 1) expect(at(image, x, y)).toEqual([expected, expected, expected, 255]);
        }
      }
    }
    for (let i = 0; i < image.data.length; i += 4) expect([0, 255]).not.toContain(image.data[i]);
  });

  it('blur from the block averages only, so pixels moved inside a block change nothing', () => {
    const first = pixels(24, 24, secret);
    const second = pixels(24, 24, (x, y) => {
      const [bx, by] = [x - (x % 8), y - (y % 8)];
      return secret(bx + 7 - (x - bx), by + 7 - (y - by));
    });
    draw.blur(first, 8);
    draw.blur(second, 8);
    expect([...first.data]).toEqual([...second.data]);
    const mosaic = pixels(24, 24, secret);
    draw.pixelate(mosaic, 8);
    expect([...first.data]).not.toEqual([...mosaic.data]);
  });

  it('black out to opaque black', () => {
    const image = pixels(3, 2, secret);
    draw.black(image);
    expect([...image.data]).toEqual(Array.from({ length: 6 }, () => [0, 0, 0, 255]).flat());
  });

  it('read and write only the redacted region of the canvas', () => {
    const calls: Call[] = [];
    draw.draw(fakeContext(calls), { type: 'redact', x: 5, y: 5, w: 100, h: 10, mode: 'pixelate', block: 4 }, 40, 30);
    expect(calls.find((call) => call.name === 'getImageData')?.args).toEqual([5, 5, 35, 10]);
    expect(calls.find((call) => call.name === 'putImageData')?.args.slice(1)).toEqual([5, 5]);
  });
});

describe('the shapes', () => {
  it('end an arrow in the head the component draws: four widths, at least 12 px', () => {
    const head = draw.arrowHead(2, 10, 58, 10, 2);
    expect(head?.base).toEqual([46, 10]);
    expect(head?.left).toEqual([46, 16]);
    expect(head?.right).toEqual([46, 4]);
    expect(draw.arrowHead(0, 0, 0.5, 0, 2)).toBeNull();
    expect(draw.arrowHead(0, 0, 100, 0, 5)?.base).toEqual([80, 0]);
  });

  it('place text by the top of its box with the embedded font', () => {
    const calls: Call[] = [];
    const ctx = fakeContext(calls);
    draw.draw(ctx, { type: 'text', x: 10, y: 20, text: 'Bug', color: '#000000', size: 20 }, 100, 100);
    expect(calls.find((call) => call.name === 'fillText')?.args).toEqual(['Bug', 10, 20 + (20 * 1069) / 1000]);
    expect(ctx.font).toBe('20px ShotAnnotation, monospace');
    expect(draw.textWidth('abcd', 10)).toBe(24);
  });

  it('draw a marker as a circle with its number centred in white', () => {
    const calls: Call[] = [];
    const ctx = fakeContext(calls);
    draw.draw(ctx, { type: 'marker', x: 50, y: 50, number: 12, color: '#e53935', size: 20 }, 100, 100);
    expect(calls.find((call) => call.name === 'arc')?.args.slice(0, 3)).toEqual([50, 50, 15]);
    const text = calls.find((call) => call.name === 'fillText');
    expect(text?.args[0]).toBe('12');
    expect(text?.args[1]).toBeCloseTo(50 - 0.6 * 16);
    expect(ctx.fillStyle).toBe('#ffffff');
  });

  it('draw every tool without failing and dim around a crop', () => {
    const items: ShotAnnotation[] = [
      { type: 'rect', x: 1, y: 1, w: 5, h: 5, color: '#ff0000', width: 2 },
      { type: 'rect', x: 1, y: 1, w: 5, h: 5, color: '#000000', width: 2, fill: true },
      { type: 'line', x1: 0, y1: 0, x2: 9, y2: 9, color: '#00ff00', width: 1 },
      { type: 'freehand', points: [[1, 1]], color: '#123456', width: 4 },
      { type: 'highlight', x: 0, y: 0, w: 4, h: 4, color: '#ffeb3b' },
      { type: 'crop', x: 2, y: 2, w: 10, h: 10 },
    ];
    const calls: Call[] = [];
    const ctx = fakeContext(calls);
    draw.render(ctx, {} as CanvasImageSource, items, 20, 20);
    expect(calls.filter((call) => call.name === 'strokeRect')).toHaveLength(1);
    expect(calls.filter((call) => call.name === 'fillRect')).toHaveLength(2);
    draw.cropFrame(ctx, items, 20, 20);
    expect(calls.filter((call) => call.name === 'fillRect')).toHaveLength(6);
    expect(draw.cropOf(items)?.type).toBe('crop');
  });
});
