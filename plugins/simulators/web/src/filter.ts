// The component's drawing, read defensively, and the CSS `filter` it becomes. Only the steps and
// layers below pass, each number finite and in range, so nothing but numbers the plugin chose
// reaches the page's style.

import type { Overlay, Spec, Step } from './types';
import { isObject } from '../../../web-common/src/is-object';

const MAX_STEPS = 8;
const AMOUNTS = new Set(['contrast', 'brightness', 'saturate']);

type Json = Record<string, unknown>;


function number(value: unknown, min: number, max: number): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max ? value : null;
}

function readStep(value: unknown): Step | null {
  if (!isObject(value)) return null;
  if (value.op === 'matrix') {
    const values = value.values;
    if (!Array.isArray(values) || values.length !== 20) return null;
    const read = values.map((entry) => number(entry, -10, 10));
    return read.every((entry) => entry !== null) ? { op: 'matrix', values: read as number[] } : null;
  }
  if (value.op === 'blur') {
    const px = number(value.px, 0, 50);
    return px === null ? null : { op: 'blur', px };
  }
  if (typeof value.op === 'string' && AMOUNTS.has(value.op)) {
    const amount = number(value.amount, 0, 4);
    return amount === null ? null : ({ op: value.op, amount } as Step);
  }
  return null;
}

function readOverlay(value: unknown): Overlay | null | undefined {
  if (value === null || value === undefined) return null;
  if (!isObject(value)) return undefined;
  const opacity = number(value.opacity, 0, 1);
  if (opacity === null) return undefined;
  if (value.kind === 'glare') return { kind: 'glare', opacity };
  if (value.kind === 'tunnel') {
    const clear = number(value.clear, 0, 1);
    const fade = number(value.fade, 0, 1);
    return clear === null || fade === null ? undefined : { kind: 'tunnel', clear, fade, opacity };
  }
  if (value.kind === 'spot') {
    const size = number(value.size, 0, 1);
    const blurPx = number(value.blurPx, 0, 50);
    return size === null || blurPx === null ? undefined : { kind: 'spot', size, blurPx, opacity };
  }
  return undefined;
}

/** The drawing in `value`, or null when there is nothing to draw or it does not read. */
export function readSpec(value: unknown): Spec | null {
  if (!isObject(value) || !Array.isArray(value.filter) || value.filter.length > MAX_STEPS) return null;
  const filter = value.filter.map(readStep);
  if (filter.some((step) => step === null)) return null;
  const overlay = readOverlay(value.overlay);
  if (overlay === undefined) return null;
  if (filter.length === 0 && overlay === null) return null;
  return { filter: filter as Step[], overlay };
}

/** The CSS `filter` of `steps`; `matrixId(index)` names the SVG filter of the index-th matrix. */
export function cssFilter(steps: readonly Step[], matrixId: (index: number) => string): string {
  let matrices = 0;
  return steps
    .map((step) => {
      switch (step.op) {
        case 'matrix':
          return `url("#${matrixId(matrices++)}")`;
        case 'blur':
          return `blur(${step.px}px)`;
        default:
          return `${step.op}(${step.amount})`;
      }
    })
    .join(' ');
}
