// The screenshot editor's input (RB-080-01): the mouse and the keyboard on the canvas. With the
// mouse a tool is dragged (or clicked, for text and markers); with the keyboard the arrow keys
// move a cursor (Shift: 10 px), Enter or Space sets the first point and Enter or Space again the
// second, and Escape drops what is in progress.
import { build, DRAGGED } from './editor';
import type { ShotAnnotation, ShotOptions } from './types';

export interface InputHooks {
  tool(): string;
  options(): ShotOptions;
  items(): ShotAnnotation[];
  commit(annotation: ShotAnnotation): void;
  preview(annotation: ShotAnnotation | null, cursor: number[] | null): void;
  askText(point: number[]): void;
  status(key: string): void;
  start(): number[];
}

export interface Input {
  /** Drops what is in progress; answers whether something was. */
  cancel(): boolean;
  /** Drops what is in progress and the keyboard cursor, which then starts at `start()`. */
  reset(): void;
  readonly busy: boolean;
}

const clamp = (value: number, low: number, high: number): number => Math.min(Math.max(value, low), high);

/**
 * The open image's size in image pixels. The canvas holds the image's pixels, or the scaled
 * preview of a large one (RB-099-24); the session names the image's own size on the canvas then
 * (`data-width`, `data-height`).
 */
export function imageSize(canvas: HTMLCanvasElement): { width: number; height: number } {
  return { width: Number(canvas.dataset.width) || canvas.width, height: Number(canvas.dataset.height) || canvas.height };
}

/** The image pixel under the pointer. */
export function toImage(canvas: HTMLCanvasElement, event: MouseEvent): number[] {
  const rect = canvas.getBoundingClientRect();
  const { width, height } = imageSize(canvas);
  const sx = rect.width > 0 ? width / rect.width : 1;
  const sy = rect.height > 0 ? height / rect.height : 1;
  return [clamp((event.clientX - rect.left) * sx, 0, width), clamp((event.clientY - rect.top) * sy, 0, height)];
}

const STEPS: Record<string, number[]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

/** Wires `canvas`; answers whether a drawing is in progress and how to drop it. */
export function attach(canvas: HTMLCanvasElement, hooks: InputHooks): Input {
  let points: number[][] = [];
  let dragging = false;
  let cursor: number[] | null = null;

  function showProgress(): void {
    const shape = points.length > 0 ? build(hooks.tool(), points, hooks.options(), hooks.items()) : null;
    hooks.preview(shape, cursor);
  }

  function cancel(): boolean {
    const busy = points.length > 0;
    points = [];
    dragging = false;
    hooks.preview(null, cursor);
    return busy;
  }

  function finish(): void {
    const tool = hooks.tool();
    const done = points;
    points = [];
    dragging = false;
    const first = done[0];
    if (tool === 'text') {
      hooks.preview(null, cursor);
      if (first) hooks.askText(first);
      return;
    }
    const annotation = build(tool, done, hooks.options(), hooks.items());
    hooks.preview(null, cursor);
    if (annotation) hooks.commit(annotation);
  }

  /** The points after the pointer or the cursor reached `point`. */
  const extend = (point: number[]): number[][] => {
    const first = points[0];
    return hooks.tool() === 'freehand' ? [...points, point] : first ? [first, point] : [point];
  };

  canvas.addEventListener('pointerdown', (event) => {
    if (event.button !== 0) return;
    canvas.focus();
    cursor = null;
    points = [toImage(canvas, event)];
    if (!DRAGGED.has(hooks.tool())) {
      finish();
      return;
    }
    dragging = true;
    if (typeof canvas.setPointerCapture === 'function') canvas.setPointerCapture(event.pointerId);
    showProgress();
  });

  canvas.addEventListener('pointermove', (event) => {
    if (!dragging) return;
    points = extend(toImage(canvas, event));
    showProgress();
  });

  canvas.addEventListener('pointerup', (event) => {
    if (!dragging) return;
    points = extend(toImage(canvas, event));
    finish();
  });

  canvas.addEventListener('keydown', (event) => {
    const step = STEPS[event.key];
    if (step && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault();
      const distance = event.shiftKey ? 10 : 1;
      const from = cursor ?? hooks.start();
      cursor = [
        clamp((from[0] ?? 0) + (step[0] ?? 0) * distance, 0, imageSize(canvas).width),
        clamp((from[1] ?? 0) + (step[1] ?? 0) * distance, 0, imageSize(canvas).height),
      ];
      if (points[0]) points = extend(cursor);
      showProgress();
      return;
    }
    if ((event.key === 'Enter' || event.key === ' ') && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      const here = cursor ?? hooks.start();
      cursor = here;
      if (!DRAGGED.has(hooks.tool())) {
        points = [here];
        finish();
      } else if (points.length === 0) {
        points = [here];
        hooks.status('panel.second_point');
        showProgress();
      } else {
        points = extend(here);
        finish();
      }
      return;
    }
    if (event.key === 'Escape' && points.length > 0) {
      event.preventDefault();
      event.stopPropagation();
      cancel();
    }
  });

  canvas.addEventListener('blur', () => {
    if (!dragging) {
      cursor = null;
      if (points.length === 0) hooks.preview(null, null);
    }
  });

  return Object.freeze({
    cancel,
    reset() {
      cancel();
      cursor = null;
    },
    get busy() {
      return points.length > 0;
    },
  });
}
