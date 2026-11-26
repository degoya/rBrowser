// The screenshot editor's drawing (RB-080-01): every annotation on a canvas with the component's
// geometry and redaction arithmetic (src/render.rs, src/redact.rs, src/text.rs), so the preview,
// the clipboard and the saved file agree. Redactions are computed into the canvas's pixels, never
// laid over them. The drawing surface stays the panel's own (docs/design.md 6, RB-080-86).
import type { ShotAnnotation, ShotPixels, ShotRegion } from './types';

const HIGHLIGHT_ALPHA = 89 / 255;
const ARROW_HEAD_WIDTHS = 4;
const ARROW_HEAD_MIN = 12;
const MARKER_RADIUS = 0.75;
const MARKER_TEXT = 0.8;
const DIGIT_HALF = 0.357;
const UNITS = 1000;
const ASCENT = 1069;
const DESCENT = -293;
const ADVANCE = 0.6;
/** The annotation font's family (panel/annotation-font.css). */
export const FONT = 'ShotAnnotation';

/** The whole pixels `x`, `y`, `w`, `h` touches inside a `width` x `height` image, or null. */
export function clip(x: number, y: number, w: number, h: number, width: number, height: number): ShotRegion | null {
  const edge = (value: number, limit: number): number => Math.min(Math.max(value, 0), limit);
  const left = edge(Math.floor(x), width);
  const top = edge(Math.floor(y), height);
  const right = edge(Math.ceil(x + w), width);
  const bottom = edge(Math.ceil(y + h), height);
  return right > left && bottom > top ? { left, top, right, bottom } : null;
}

const average = (sum: number, count: number): number => Math.floor((sum + Math.floor(count / 2)) / count);

/** Every block of `block` pixels (from the top left) becomes its rounded average. */
export function pixelate(image: ShotPixels, block: number): void {
  const { data, width, height } = image;
  const step = Math.max(1, block);
  for (let top = 0; top < height; top += step) {
    const bottom = Math.min(top + step, height);
    for (let left = 0; left < width; left += step) {
      const right = Math.min(left + step, width);
      const sum = [0, 0, 0, 0];
      for (let y = top; y < bottom; y += 1) {
        for (let x = left; x < right; x += 1) {
          const i = (y * width + x) * 4;
          for (let c = 0; c < 4; c += 1) sum[c] = (sum[c] ?? 0) + (data[i + c] ?? 0);
        }
      }
      const count = (right - left) * (bottom - top);
      const mean = sum.map((total) => average(total, count));
      for (let y = top; y < bottom; y += 1) {
        for (let x = left; x < right; x += 1) data.set(mean, (y * width + x) * 4);
      }
    }
  }
}

/** One box-blur pass of `radius` along rows or columns, edge pixels repeated. */
function boxPass(image: ShotPixels, radius: number, horizontal: boolean): void {
  const { data, width, height } = image;
  const [outer, inner] = horizontal ? [height, width] : [width, height];
  const line = new Uint8ClampedArray(inner * 4);
  const at = (o: number, i: number): number => (horizontal ? o * width + i : i * width + o) * 4;
  for (let o = 0; o < outer; o += 1) {
    for (let i = 0; i < inner; i += 1) line.set(data.subarray(at(o, i), at(o, i) + 4), i * 4);
    for (let i = 0; i < inner; i += 1) {
      const sum = [0, 0, 0, 0];
      for (let offset = -radius; offset <= radius; offset += 1) {
        const from = Math.min(Math.max(i + offset, 0), inner - 1) * 4;
        for (let c = 0; c < 4; c += 1) sum[c] = (sum[c] ?? 0) + (line[from + c] ?? 0);
      }
      data.set(sum.map((total) => average(total, 2 * radius + 1)), at(o, i));
    }
  }
}

/** The mosaic, smoothed by half a block: built from the block averages only. */
export function blur(image: ShotPixels, block: number): void {
  pixelate(image, block);
  const radius = Math.max(1, Math.floor(block / 2));
  boxPass(image, radius, true);
  boxPass(image, radius, false);
}

export function black(image: ShotPixels): void {
  for (let i = 0; i < image.data.length; i += 4) image.data.set([0, 0, 0, 255], i);
}

/** The arrow's shaft end and head, as the component draws them; null for a dot. */
export function arrowHead(x1: number, y1: number, x2: number, y2: number, width: number): { base: number[]; left: number[]; right: number[] } | null {
  const length = Math.hypot(x2 - x1, y2 - y1);
  if (length < 1) return null;
  const [ux, uy] = [(x2 - x1) / length, (y2 - y1) / length];
  const head = Math.min(Math.max(width * ARROW_HEAD_WIDTHS, ARROW_HEAD_MIN), length);
  const base = [x2 - ux * head, y2 - uy * head];
  const half = head / 2;
  return {
    base,
    left: [(base[0] ?? 0) - uy * half, (base[1] ?? 0) + ux * half],
    right: [(base[0] ?? 0) + uy * half, (base[1] ?? 0) - ux * half],
  };
}

export const font = (size: number): string => `${size}px ${FONT}, monospace`;
export const textWidth = (text: string, size: number): number => [...text].length * ADVANCE * size;
export const lineHeight = (size: number): number => (size * (ASCENT - DESCENT)) / UNITS;

function words(ctx: CanvasRenderingContext2D, text: string, x: number, top: number, size: number, color: string): void {
  ctx.font = font(size);
  ctx.textBaseline = 'alphabetic';
  ctx.fillStyle = color;
  ctx.fillText(text, x, top + (size * ASCENT) / UNITS);
}

function polyline(ctx: CanvasRenderingContext2D, points: number[][], color: string, width: number): void {
  const [first, ...rest] = points;
  if (!first) return;
  ctx.fillStyle = color;
  ctx.strokeStyle = color;
  if (rest.length === 0) {
    ctx.beginPath();
    ctx.arc(first[0] ?? 0, first[1] ?? 0, width / 2, 0, Math.PI * 2);
    ctx.fill();
    return;
  }
  ctx.lineWidth = width;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.beginPath();
  ctx.moveTo(first[0] ?? 0, first[1] ?? 0);
  for (const point of rest) ctx.lineTo(point[0] ?? 0, point[1] ?? 0);
  ctx.stroke();
}

/**
 * Draws one annotation of a `width` x `height` image; a redaction reads and writes the pixels
 * under it. `scale` image pixels make one canvas pixel (RB-099-24, a large image's preview): the
 * caller sets that transform for the shapes, and a redaction - which reads the canvas's own
 * pixels - works on the canvas pixels under it, its blocks scaled as far.
 */
export function draw(ctx: CanvasRenderingContext2D, a: ShotAnnotation, width: number, height: number, scale = 1): void {
  const color = a.color ?? '#000000';
  const [x, y, w, h] = [a.x ?? 0, a.y ?? 0, a.w ?? 0, a.h ?? 0];
  ctx.save();
  if (a.type === 'rect' && a.fill) {
    ctx.fillStyle = color;
    ctx.fillRect(x, y, w, h);
  } else if (a.type === 'rect') {
    ctx.strokeStyle = color;
    ctx.lineWidth = a.width ?? 1;
    ctx.lineJoin = 'miter';
    ctx.strokeRect(x, y, w, h);
  } else if (a.type === 'line' || a.type === 'arrow') {
    const [x1, y1, x2, y2] = [a.x1 ?? 0, a.y1 ?? 0, a.x2 ?? 0, a.y2 ?? 0];
    const head = a.type === 'arrow' ? arrowHead(x1, y1, x2, y2, a.width ?? 1) : null;
    if (a.type === 'arrow' && !head) {
      ctx.restore();
      return;
    }
    polyline(ctx, [[x1, y1], head ? head.base : [x2, y2]], color, a.width ?? 1);
    if (head) {
      ctx.beginPath();
      ctx.moveTo(x2, y2);
      ctx.lineTo(head.left[0] ?? 0, head.left[1] ?? 0);
      ctx.lineTo(head.right[0] ?? 0, head.right[1] ?? 0);
      ctx.closePath();
      ctx.fill();
    }
  } else if (a.type === 'freehand') {
    polyline(ctx, a.points ?? [], color, a.width ?? 1);
  } else if (a.type === 'text') {
    words(ctx, a.text ?? '', x, y, a.size ?? 16, color);
  } else if (a.type === 'marker') {
    const size = a.size ?? 16;
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x, y, size * MARKER_RADIUS, 0, Math.PI * 2);
    ctx.fill();
    const label = String(a.number ?? 1);
    const em = size * MARKER_TEXT;
    words(ctx, label, x - textWidth(label, em) / 2, y + DIGIT_HALF * em - (em * ASCENT) / UNITS, em, '#ffffff');
  } else if (a.type === 'highlight') {
    ctx.globalAlpha = HIGHLIGHT_ALPHA;
    ctx.fillStyle = color;
    ctx.fillRect(x, y, w, h);
  } else if (a.type === 'redact') {
    const region = clip(x / scale, y / scale, w / scale, h / scale, Math.ceil(width / scale), Math.ceil(height / scale));
    if (region) {
      const pixels = ctx.getImageData(region.left, region.top, region.right - region.left, region.bottom - region.top);
      const block = Math.max(1, Math.round((a.block ?? 12) / scale));
      if (a.mode === 'black') black(pixels);
      else if (a.mode === 'blur') blur(pixels, block);
      else pixelate(pixels, block);
      ctx.putImageData(pixels, region.left, region.top);
    }
  }
  ctx.restore();
}

export const cropOf = (items: readonly ShotAnnotation[]): ShotAnnotation | null => items.find((item) => item.type === 'crop') ?? null;

/**
 * The image with every annotation, the crop not applied: `source` at the canvas's own size, the
 * annotations in the pixels of the `width` x `height` image, `scale` of them to a canvas pixel.
 */
export function render(ctx: CanvasRenderingContext2D, source: CanvasImageSource, items: readonly ShotAnnotation[], width: number, height: number, scale = 1): void {
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, Math.ceil(width / scale), Math.ceil(height / scale));
  ctx.drawImage(source, 0, 0);
  ctx.setTransform(1 / scale, 0, 0, 1 / scale, 0, 0);
  for (const item of items) draw(ctx, item, width, height, scale);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

/** The crop of `items` as a frame, the rest dimmed; `unit` image pixels make one canvas pixel. */
export function cropFrame(ctx: CanvasRenderingContext2D, items: readonly ShotAnnotation[], width: number, height: number, unit = 1): void {
  const crop = cropOf(items);
  const region = crop && clip(crop.x ?? 0, crop.y ?? 0, crop.w ?? 0, crop.h ?? 0, width, height);
  if (!region) return;
  ctx.save();
  ctx.fillStyle = 'rgba(0, 0, 0, 0.45)';
  ctx.fillRect(0, 0, width, region.top);
  ctx.fillRect(0, region.bottom, width, height - region.bottom);
  ctx.fillRect(0, region.top, region.left, region.bottom - region.top);
  ctx.fillRect(region.right, region.top, width - region.right, region.bottom - region.top);
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = unit;
  ctx.setLineDash([6 * unit, 4 * unit]);
  const half = unit / 2;
  ctx.strokeRect(region.left + half, region.top + half, region.right - region.left - unit, region.bottom - region.top - unit);
  ctx.restore();
}

/** The keyboard cursor: a square, white under black, seen on any image; `unit` as above. */
export function cursorMark(ctx: CanvasRenderingContext2D, x: number, y: number, unit = 1): void {
  ctx.save();
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 3 * unit;
  ctx.strokeRect(x - 6 * unit, y - 6 * unit, 12 * unit, 12 * unit);
  ctx.strokeStyle = '#000000';
  ctx.lineWidth = unit;
  ctx.strokeRect(x - 6 * unit, y - 6 * unit, 12 * unit, 12 * unit);
  ctx.restore();
}
