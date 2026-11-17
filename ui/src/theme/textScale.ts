// Windows' text size (RB-090-26, WCAG 1.4.4): Settings > Accessibility > Text size, 100 to
// 225 %, as the core reads it (`crates/rbrowser/src/text_scale/`) while
// `ui.followWindowsTextSize` is on. The shell's pages take it as their root font size, so every
// rem of Tailwind and Nuxt UI - text, spacing, the toolbar's `h-11` row - grows with it, and the
// native strips the core sizes grow by the same factor. The page zoom stays at none
// (`zoomGuard.ts`), so a CSS px stays a DIP and the canvas board's frames stay on the views.

/** The browser's root font size at 100 %, in px: the size Tailwind's rem is drawn for. */
export const BASE_FONT_PX = 16;
const MIN_PERCENT = 100;
const MAX_PERCENT = 225;

/** The core's percentage, held to 100..225; anything else is 100. */
export function parseTextScale(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value)) {
    return MIN_PERCENT;
  }
  return Math.min(MAX_PERCENT, Math.max(MIN_PERCENT, Math.round(value)));
}

/** Draws the page at `percent`: the root's font size; at 100 % the page's own. */
export function applyTextScale(root: HTMLElement, percent: number): void {
  root.style.fontSize = percent === MIN_PERCENT ? '' : `${percent}%`;
}

/** One rem in px over its size at 100 %: what a width measured for `w-8` or `gap-2` grows by. */
export function remScale(
  root: HTMLElement = document.documentElement,
  style: (element: HTMLElement) => CSSStyleDeclaration = (element) => getComputedStyle(element),
): number {
  const size = style(root).fontSize;
  const px = size.endsWith('px') ? Number.parseFloat(size) : Number.NaN;
  return Number.isFinite(px) && px > 0 ? px / BASE_FONT_PX : 1;
}
