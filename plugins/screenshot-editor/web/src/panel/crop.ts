// The screenshot editor's crop (RB-080-63): the crop tool drags a frame that is a draft until it
// is applied - Apply (Enter) commits it as the crop annotation, Cancel (Escape) drops it, Undo
// takes an applied crop back. The frame has eight handles to resize it and moves when dragged
// inside. This file holds the frame's geometry and its pointer and Enter handling on the canvas;
// the session (`session.ts`) keeps the draft.
import { clip } from './draw';
import { imageSize, toImage } from './input';
import type { ShotAnnotation, ShotRegion } from './types';

/** The handles, clockwise from the top left. */
export const HANDLES: readonly string[] = Object.freeze(['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']);
/** A handle's size and grab distance, in CSS pixels. */
export const HANDLE_PX = 8;
const CURSORS: Record<string, string> = {
  nw: 'nwse-resize',
  se: 'nwse-resize',
  ne: 'nesw-resize',
  sw: 'nesw-resize',
  n: 'ns-resize',
  s: 'ns-resize',
  e: 'ew-resize',
  w: 'ew-resize',
  move: 'move',
};

const clamp = (value: number, low: number, high: number): number => Math.min(Math.max(value, low), high);
const round = (value: number): number => Math.round(value * 10) / 10;

/** Each handle's centre. */
function points(frame: ShotAnnotation): Record<string, number[]> {
  const [x, y, w, h] = [frame.x ?? 0, frame.y ?? 0, frame.w ?? 0, frame.h ?? 0];
  return { nw: [x, y], n: [x + w / 2, y], ne: [x + w, y], e: [x + w, y + h / 2], se: [x + w, y + h], s: [x + w / 2, y + h], sw: [x, y + h], w: [x, y + h / 2] };
}

/** The handle at `point` (within `tolerance` image pixels), `move` inside the frame, else null. */
export function hit(frame: ShotAnnotation, point: number[], tolerance: number): string | null {
  const [px, py] = [point[0] ?? 0, point[1] ?? 0];
  const at = points(frame);
  for (const handle of HANDLES) {
    const [hx, hy] = at[handle] ?? [0, 0];
    if (Math.abs(px - (hx ?? 0)) <= tolerance && Math.abs(py - (hy ?? 0)) <= tolerance) return handle;
  }
  const [x, y, w, h] = [frame.x ?? 0, frame.y ?? 0, frame.w ?? 0, frame.h ?? 0];
  return px > x && px < x + w && py > y && py < y + h ? 'move' : null;
}

/**
 * `frame` with `handle` dragged from `from` to `to`, inside a `width` x `height` image; the old
 * frame when the new one would be empty.
 */
export function drag(frame: ShotAnnotation, handle: string, from: number[], to: number[], width: number, height: number): ShotAnnotation {
  const [dx, dy] = [(to[0] ?? 0) - (from[0] ?? 0), (to[1] ?? 0) - (from[1] ?? 0)];
  const [x, y, w, h] = [frame.x ?? 0, frame.y ?? 0, frame.w ?? 0, frame.h ?? 0];
  if (handle === 'move') {
    return { type: 'crop', x: round(clamp(x + dx, 0, Math.max(width - w, 0))), y: round(clamp(y + dy, 0, Math.max(height - h, 0))), w, h };
  }
  let [left, top, right, bottom] = [x, y, x + w, y + h];
  if (handle.includes('w')) left = clamp(left + dx, 0, width);
  if (handle.includes('e')) right = clamp(right + dx, 0, width);
  if (handle.includes('n')) top = clamp(top + dy, 0, height);
  if (handle.includes('s')) bottom = clamp(bottom + dy, 0, height);
  const next = {
    type: 'crop',
    x: round(Math.min(left, right)),
    y: round(Math.min(top, bottom)),
    w: round(Math.abs(right - left)),
    h: round(Math.abs(bottom - top)),
  };
  return next.w >= 1 && next.h >= 1 ? next : frame;
}

/** Whether two frames are the same. */
export const same = (a: ShotAnnotation | null, b: ShotAnnotation | null): boolean =>
  !!a && !!b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;

/** The frame's size in whole pixels, as the export will have it. */
export function size(frame: ShotAnnotation, width: number, height: number): { width: number; height: number } {
  const region = clip(frame.x ?? 0, frame.y ?? 0, frame.w ?? 0, frame.h ?? 0, width, height);
  return region ? { width: region.right - region.left, height: region.bottom - region.top } : { width: 0, height: 0 };
}

/** The handles as small squares; `scale` is image pixels per CSS pixel. */
export function drawHandles(ctx: CanvasRenderingContext2D, frame: ShotAnnotation, scale: number): void {
  const half = (HANDLE_PX / 2) * scale;
  ctx.save();
  ctx.setLineDash([]);
  ctx.lineWidth = Math.max(scale, 1);
  for (const [hx, hy] of Object.values(points(frame))) {
    ctx.fillStyle = '#ffffff';
    ctx.fillRect((hx ?? 0) - half, (hy ?? 0) - half, half * 2, half * 2);
    ctx.strokeStyle = '#1d2129';
    ctx.strokeRect((hx ?? 0) - half, (hy ?? 0) - half, half * 2, half * 2);
  }
  ctx.restore();
}

/**
 * Puts `bar` (Apply, Cancel, the size) under the frame, or inside its bottom edge when the image
 * ends there; `zoom` is CSS pixels per image pixel.
 */
export function place(bar: HTMLElement, frame: ShotAnnotation, zoom: number, canvas: HTMLCanvasElement): void {
  // Layout read first, then written (review of RB-080-63: no forced reflow between).
  const [height, left, top] = [bar.offsetHeight, canvas.offsetLeft, canvas.offsetTop];
  const bottom = ((frame.y ?? 0) + (frame.h ?? 0)) * zoom;
  const room = imageSize(canvas).height * zoom - height - 4;
  bar.style.left = `${left + Math.max((frame.x ?? 0) * zoom, 0)}px`;
  bar.style.top = `${top + Math.max(Math.min(bottom + 6, room), 0)}px`;
}

/** The middle of `region`, where the keyboard cursor starts. */
export const middle = (region: ShotRegion): number[] => [Math.round((region.left + region.right) / 2), Math.round((region.top + region.bottom) / 2)];

/** The canvas at `zoom` in `frame`, which shows only `region` (the applied crop) when one is given. */
export function view(frame: HTMLElement, canvas: HTMLCanvasElement, region: ShotRegion | null, image: { width: number; height: number }, zoom: number): void {
  const px = (value: number): string => `${Math.round(value * zoom)}px`;
  canvas.style.width = px(image.width);
  canvas.style.height = px(image.height);
  frame.classList.toggle('cropped', region !== null);
  frame.style.width = region ? px(region.right - region.left) : '';
  frame.style.height = region ? px(region.bottom - region.top) : '';
  canvas.style.marginLeft = region ? px(-region.left) : '';
  canvas.style.marginTop = region ? px(-region.top) : '';
}

export interface CropHooks {
  active(): boolean;
  frame(): ShotAnnotation | null;
  pending(): boolean;
  idle(): boolean;
  change(frame: ShotAnnotation): void;
  apply(): void;
}

/**
 * Wires the draft's handles and Enter on `canvas`. Attached before the drawing input
 * (`input.ts`), so a grab on the frame or Enter on a changed draft goes no further; a press
 * outside the frame, or Enter on the crop as applied, draws a new one.
 */
export function attach(canvas: HTMLCanvasElement, hooks: CropHooks): void {
  let grab: { handle: string; from: number[]; frame: ShotAnnotation } | null = null;
  const scale = (): number => {
    const rect = canvas.getBoundingClientRect();
    return rect.width > 0 ? imageSize(canvas).width / rect.width : 1;
  };
  const handleAt = (event: MouseEvent): string | null => {
    const frame = hooks.active() ? hooks.frame() : null;
    return frame ? hit(frame, toImage(canvas, event), HANDLE_PX * scale()) : null;
  };

  canvas.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    const frame = hooks.active() ? hooks.frame() : null;
    const handle = handleAt(event);
    if (!frame || !handle) return;
    event.stopImmediatePropagation();
    event.preventDefault();
    canvas.focus();
    grab = { handle, from: toImage(canvas, event), frame };
    if (typeof canvas.setPointerCapture === 'function') canvas.setPointerCapture(event.pointerId);
  });

  const move = (event: PointerEvent): boolean => {
    if (!grab) {
      canvas.style.cursor = CURSORS[handleAt(event) ?? ''] ?? '';
      return false;
    }
    event.stopImmediatePropagation();
    const { width, height } = imageSize(canvas);
    hooks.change(drag(grab.frame, grab.handle, grab.from, toImage(canvas, event), width, height));
    return true;
  };
  canvas.addEventListener('pointermove', move);
  canvas.addEventListener('pointerup', (event) => {
    if (move(event)) grab = null;
  });
  // Review of RB-080-63: a cancelled pointer ends the grab, or hover moves keep resizing.
  const drop = (): void => {
    grab = null;
  };
  canvas.addEventListener('pointercancel', drop);
  canvas.addEventListener('lostpointercapture', drop);

  canvas.addEventListener('keydown', (event) => {
    if (event.key !== 'Enter' || event.ctrlKey || event.metaKey || event.altKey) return;
    if (!hooks.active() || !hooks.pending() || !hooks.idle()) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    hooks.apply();
  });
}
