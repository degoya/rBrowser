// The page part as the component runs it: built into ../src/page.js as the global `FontsPage`,
// evaluated in the plugin's isolated world of each view (`FontsPage.run({...})`,
// `FontsPage.mark({...})`, `FontsPage.clear()`; src/page.rs). Every request is data the
// component wrote as JSON; a malformed one reads as the defaults.

import { collect } from './collect';
import { clear as clearMarks, mark as markFamily } from './mark';
import type { MarkRequest, PageFonts, Request } from './types';

function object(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

function finite(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

/** One view's fonts for `request`. */
export function run(request: unknown): PageFonts {
  const typed = object(request);
  const clean: Request = {
    minTextPx: finite(typed.minTextPx),
    minInputPx: finite(typed.minInputPx),
    minLineHeight: finite(typed.minLineHeight),
  };
  return collect(clean);
}

/** Marks the text set in `request.family` (and `weight`, `style`). */
export function mark(request: unknown): { count: number } {
  const typed = object(request);
  const clean: MarkRequest = {
    family: typeof typed.family === 'string' ? typed.family.slice(0, 200) : undefined,
    weight: finite(typed.weight),
    style: typeof typed.style === 'string' ? typed.style.slice(0, 40) : undefined,
  };
  return markFamily(clean);
}

/** Removes the marks. */
export function clear(): { cleared: boolean } {
  clearMarks();
  return { cleared: true };
}
