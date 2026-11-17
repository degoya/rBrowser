// Computed colours for the component, which reads sRGB only (`rgb()`, `rgba()`): Chromium keeps
// a colour given as `oklch()`, `lab()` or `color(display-p3 ...)` in that space, so those are
// painted into a 1x1 canvas - never attached to the page - and read back as sRGB bytes. Without a
// canvas the text goes as it is, and the component calls the contrast unknown.

const SRGB = /^(rgba?\(|transparent$|#)/i;

let context: OffscreenCanvasRenderingContext2D | null | undefined;

function canvas(): OffscreenCanvasRenderingContext2D | null {
  if (context === undefined) {
    try {
      context =
        typeof OffscreenCanvas === 'function'
          ? new OffscreenCanvas(1, 1).getContext('2d', { willReadFrequently: true })
          : null;
    } catch {
      context = null;
    }
  }
  return context;
}

/** `text` as `rgb()`/`rgba()` when it is in another colour space and a canvas can tell. */
export function toSrgb(text: string): string {
  const trimmed = text.trim();
  if (SRGB.test(trimmed)) return trimmed;
  const ctx = canvas();
  if (!ctx) return trimmed;
  ctx.clearRect(0, 0, 1, 1);
  ctx.fillStyle = '#000';
  ctx.fillStyle = trimmed;
  ctx.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
  if (r === undefined || g === undefined || b === undefined || a === undefined) return trimmed;
  return a === 255 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${Math.round((a / 255) * 1000) / 1000})`;
}

/** Whether a computed `background-color` (after [`toSrgb`]) lets nothing through. */
export function isOpaque(color: string): boolean {
  return (/^rgb\(/i.test(color) && !color.includes('/')) || /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(color);
}
