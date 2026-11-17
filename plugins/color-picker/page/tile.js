// @ts-check
// The colour picker's tiles (RB-080-77, split from page/picker.js): a `tile` answer of the
// component read into a tile, and the image pixel under a viewport point in it - as
// `Mapping::point` in src/pixels.rs: of the window's image, by the measured offset and scale of the
// viewport in it; of a clip, by the clip's origin in the document. A frozen global of the
// plugin's isolated world, `rbColorPickerTile`, which page/picker.js (run after it) draws and
// places the loupe by (`place`: inside the part of the view known to be seen, RB-090-40); the
// page never sees it.
(() => {
  'use strict';
  if (typeof rbPlugin === 'undefined' || window !== window.top) return;

  /** A `tile` answer as a tile: cut from the window's image (`left`) or a clip (`x`); `null` when it does not read. @param {any} answer @returns {ColorPickerTile | null} */
  function read(answer) {
    const clipped = typeof answer?.left !== 'number';
    const keys = clipped ? ['x', 'y'] : ['left', 'top', 'imageWidth', 'imageHeight'];
    const [first = -1, second = -1, imageWidth = Infinity, imageHeight = Infinity] = keys.map((key) => Number(answer?.[key]));
    const [scale = 0, width = 0, height = 0, offsetX = 0, offsetY = 0] = ['scale', 'width', 'height', 'offsetX', 'offsetY'].map((key) => Number(answer?.[key] ?? 0));
    const [left, top, originX, originY] = clipped ? [0, 0, first, second] : [first, second, 0, 0];
    if (typeof answer?.rgb !== 'string' || ![first, second].every((n) => Number.isFinite(n) && n >= 0)) return null;
    if (![offsetX, offsetY].every(Number.isInteger)) return null;
    if (!(scale > 0 && width > 0 && height > 0 && imageWidth >= left + width && imageHeight >= top + height)) return null;
    let rgb = new Uint8Array();
    try {
      rgb = Uint8Array.from(atob(answer.rgb), (char) => char.charCodeAt(0));
    } catch {
      return null;
    }
    const tile = { originX, originY, offsetX, offsetY, document: clipped, left, top, scale, width, height, imageWidth, imageHeight, rgb, at: Date.now() };
    return rgb.length === width * height * 3 ? tile : null;
  }

  /** The tile's pixel under a viewport point: the image pixel in the middle of its CSS pixel. @param {ColorPickerTile} of @param {{ x: number, y: number }} at */
  function pixel(of, at) {
    const image = (/** @type {number} */ point, /** @type {number} */ scroll, /** @type {number} */ origin, /** @type {number} */ offset) =>
      offset + Math.floor((Math.floor(point + (of.document ? Math.max(0, scroll) : 0)) + 0.5 - origin) * of.scale);
    return { px: image(at.x, window.scrollX, of.originX, of.offsetX) - of.left, py: image(at.y, window.scrollY, of.originY, of.offsetY) - of.top };
  }

  /** Whether the tile holds `half` pixels around `at`; at the image's, the viewport's or the document's edge a part. @param {ColorPickerTile} of @param {{ x: number, y: number }} at @param {number} half */
  function holds(of, at, half) {
    const { px, py } = pixel(of, at);
    const left = of.left <= Math.max(0, of.offsetX) && of.originX === 0 ? 0 : half;
    const top = of.top <= Math.max(0, of.offsetY) && of.originY === 0 ? 0 : half;
    const right = of.left + of.width >= of.imageWidth ? 0 : half;
    const bottom = of.top + of.height >= of.imageHeight ? 0 : half;
    return px >= left && py >= top && px < of.width - right && py < of.height - bottom;
  }

  /** The loupe's `size` x `size` cells at `at` (`#rrggbb`), `null` outside the tile; its middle is the pixel under `at`. @param {ColorPickerTile} of @param {{ x: number, y: number }} at @param {number} size */
  function grid(of, at, size) {
    const { px, py } = pixel(of, at);
    const half = Math.floor(size / 2);
    const hex = (/** @type {number} */ index) => `#${[0, 1, 2].map((c) => (of.rgb[index + c] ?? 0).toString(16).padStart(2, '0')).join('')}`;
    return Array.from({ length: size * size }, (_, index) => {
      const cx = px + (index % size) - half;
      const cy = py + Math.floor(index / size) - half;
      return cx < 0 || cy < 0 || cx >= of.width || cy >= of.height ? null : hex((cy * of.width + cx) * 3);
    });
  }

  /** `reach` grown by the point `at`: where the pointer has been, the view is seen. @param {ColorPickerArea | null} reach @param {{ x: number, y: number }} at @returns {ColorPickerArea} */
  function widen(reach, at) {
    if (!reach) return { left: at.x, top: at.y, right: at.x, bottom: at.y };
    return { left: Math.min(reach.left, at.x), top: Math.min(reach.top, at.y), right: Math.max(reach.right, at.x), bottom: Math.max(reach.bottom, at.y) };
  }

  /**
   * Where the loupe of `box` goes for the pointer `at` (RB-090-40, diag13): `gap` below right of
   * it, flipped left (above) where the part of the view known to be seen has no room - `seen` as
   * the component measured it (src/visible.rs; anything else: the viewport), widened by `reach`,
   * inside the viewport; with room on neither side, the side with more. CSS px of the viewport.
   * @param {{ x: number, y: number }} at @param {{ width: number, height: number }} box @param {number} gap @param {unknown} seen @param {ColorPickerArea | null} reach
   */
  function place(at, box, gap, seen, reach) {
    const given = /** @type {any} */ (seen);
    const valid = ['left', 'top', 'right', 'bottom'].every((key) => Number.isFinite(given?.[key]));
    let area = valid ? /** @type {ColorPickerArea} */ ({ left: given.left, top: given.top, right: given.right, bottom: given.bottom }) : null;
    if (area && reach) area = widen(widen(area, { x: reach.left, y: reach.top }), { x: reach.right, y: reach.bottom });
    const [width, height] = [window.innerWidth, window.innerHeight];
    const { left, top, right, bottom } = area ?? { left: 0, top: 0, right: width, bottom: height };
    const axis = (/** @type {number} */ point, /** @type {number} */ size, /** @type {number} */ low, /** @type {number} */ high) => {
      const [after, before] = [point + gap, point - gap - size];
      if (after + size <= high) return after;
      if (before >= low) return before;
      return high - point >= point - low ? after : before;
    };
    const x = axis(at.x, box.width, Math.max(0, left), Math.min(width, right));
    const y = axis(at.y, box.height, Math.max(0, top), Math.min(height, bottom));
    return { left: Math.max(0, x), top: Math.max(0, y), width: box.width, height: box.height };
  }

  globalThis.rbColorPickerTile = Object.freeze({ read, pixel, holds, grid, widen, place });
})();
