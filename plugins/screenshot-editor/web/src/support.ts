// The screenshot editor's unit tests: a canvas context that records its calls, and pixels.
import type { ShotPixels } from './panel/types';

export interface Call {
  name: string;
  args: unknown[];
}

/** A 2D context that records every call; `getImageData` answers white opaque pixels. */
export function fakeContext(calls: Call[] = []): CanvasRenderingContext2D {
  const record =
    (name: string) =>
    (...args: unknown[]) => {
      calls.push({ name, args });
    };
  const context: Record<string, unknown> = {
    calls,
    save: record('save'),
    restore: record('restore'),
    clearRect: record('clearRect'),
    drawImage: record('drawImage'),
    fillRect: record('fillRect'),
    strokeRect: record('strokeRect'),
    beginPath: record('beginPath'),
    moveTo: record('moveTo'),
    lineTo: record('lineTo'),
    arc: record('arc'),
    closePath: record('closePath'),
    fill: record('fill'),
    stroke: record('stroke'),
    fillText: record('fillText'),
    setLineDash: record('setLineDash'),
    getImageData(x: number, y: number, width: number, height: number) {
      calls.push({ name: 'getImageData', args: [x, y, width, height] });
      return { data: new Uint8ClampedArray(width * height * 4).fill(255), width, height };
    },
    putImageData: record('putImageData'),
  };
  return context as unknown as CanvasRenderingContext2D;
}

/** `width` x `height` straight RGBA pixels, each `pixel(x, y)`. */
export function pixels(width: number, height: number, pixel: (x: number, y: number) => number[]): ShotPixels {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) data.set(pixel(x, y), (y * width + x) * 4);
  }
  return { data, width, height };
}
