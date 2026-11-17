// Fake answers of the component for the page-script tests (RB-080-56, RB-080-64, RB-080-77):
// `calibrate` measures the view's window image - the viewport at `scale` image pixels per CSS px,
// from the image pixel `offset` -, and `tile` cuts it to `TILE_HALF` image pixels around the
// pointer by the map the page script sends, as `tile` in src/sample.rs cuts it, or (`clip`) a
// screenshot of a clip of the document as `tile_around` in src/pixels.rs sizes it; `TILE_HALF`
// is read from src/pixels.rs, so the two cannot drift.
import pixels from '../../src/pixels.rs?raw';

import type { Message } from './support';

const declared = /pub const TILE_HALF: u32 = ([0-9]+);/.exec(pixels);
if (!declared?.[1]) throw new Error('TILE_HALF is not declared in src/pixels.rs');
/** Half the tile's side in image pixels. */
export const TILE_HALF = Number(declared[1]);

export interface TileOptions {
  /** How Chromium rounds the viewport's size at the DPR into the image's. */
  round?: (pixels: number) => number;
  /** The colour of an image pixel, from the CSS pixel it shows (of the viewport; with `clip` of the document). */
  paint?: (cssX: number, cssY: number) => [number, number, number];
  /** The settings' clips: a screenshot of a clip of the document, not the window's image. */
  clip?: boolean;
  /** Where the viewport starts in the window's image (image pixels). */
  offset?: [number, number];
}

/** The window image's size and scale for a message's viewport, as Chromium rounds them. */
function windowOf(message: Message, options: TileOptions): { imageWidth: number; imageHeight: number; scale: number } {
  const dpr = Number(message.dpr) || 1;
  const [cssWidth, cssHeight] = [Number(message.width), Number(message.height)];
  const round = options.round ?? Math.round;
  const [offsetX, offsetY] = options.offset ?? [0, 0];
  const [imageWidth, imageHeight] = [round(cssWidth * dpr) + 2 * offsetX, round(cssHeight * dpr) + 2 * offsetY];
  // As `calibrate` in src/calibrate.rs snaps the markers' scale: the DPR when the image is the viewport at it, rounded.
  const scale = Math.abs(round(cssWidth * dpr) - cssWidth * dpr) <= 1 ? dpr : round(cssWidth * dpr) / cssWidth;
  return { imageWidth, imageHeight, scale };
}

/** The `calibrate` answer: the map of the view's window image; `exact: false` for a display's colours. */
export function calibrateAnswer(message: Message, options: TileOptions = {}, exact = true): unknown {
  const { imageWidth, imageHeight, scale } = windowOf(message, options);
  const [left, top] = options.offset ?? [0, 0];
  return { map: { left, top, scale, width: imageWidth, height: imageHeight }, exact };
}

const BLUE: [number, number, number] = [0x1e, 0x88, 0xe5];

export function tileAnswer(message: Message, options: TileOptions = {}): unknown {
  if (options.clip || message.clip === true) return clipTileAnswer(message, options);
  const map = message.map as { left: number; top: number; scale: number; width: number; height: number } | undefined;
  if (!map) return new Error('plugins.error.invalid');
  const { left: offsetX, top: offsetY, scale, width: imageWidth, height: imageHeight } = map;
  const [pointX, pointY] = [Math.floor(Number(message.x)), Math.floor(Number(message.y))];
  const [centreX, centreY] = [offsetX + Math.floor((pointX + 0.5) * scale), offsetY + Math.floor((pointY + 0.5) * scale)];
  const left = Math.max(0, centreX - TILE_HALF);
  const top = Math.max(0, centreY - TILE_HALF);
  const width = Math.min(imageWidth, centreX + TILE_HALF + 1) - left;
  const height = Math.min(imageHeight, centreY + TILE_HALF + 1) - top;
  const paint = options.paint ?? ((cx: number, cy: number) => (cx === pointX && cy === pointY ? BLUE : [255, 255, 255]));
  const bytes = new Uint8Array(width * height * 3);
  for (let row = 0; row < height; row += 1) {
    for (let column = 0; column < width; column += 1) {
      // An image pixel shows the CSS pixel under its centre; outside the viewport the frame.
      const css = (image: number, offset: number) => Math.floor((image + 0.5 - offset) / scale);
      const [cx, cy] = [css(left + column, offsetX), css(top + row, offsetY)];
      const frame = cx < 0 || cy < 0;
      bytes.set(frame ? [31, 31, 31] : paint(cx, cy), (row * width + column) * 3);
    }
  }
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return { left, top, offsetX, offsetY, scale, width, height, imageWidth, imageHeight, rgb: btoa(binary) };
}

/** A clip's tile: a clip of the document around the pointer, `x`, `y` its origin. */
function clipTileAnswer(message: Message, options: TileOptions): unknown {
  const dpr = Number(message.dpr) || 1;
  const docX = Math.floor(Number(message.x) + Number(message.scrollX));
  const docY = Math.floor(Number(message.y) + Number(message.scrollY));
  const half = Math.max(1, Math.floor(TILE_HALF / dpr));
  const side = 2 * half + 1;
  const x = Math.max(0, docX - half);
  const y = Math.max(0, docY - half);
  const size = (options.round ?? Math.round)(side * dpr);
  const paint = options.paint ?? ((cx: number, cy: number) => (cx === docX && cy === docY ? BLUE : [255, 255, 255]));
  const bytes = new Uint8Array(size * size * 3);
  for (let row = 0; row < size; row += 1) {
    for (let column = 0; column < size; column += 1) {
      // A device pixel shows the CSS pixel under its centre.
      bytes.set(paint(x + Math.floor((column + 0.5) / dpr), y + Math.floor((row + 0.5) / dpr)), (row * size + column) * 3);
    }
  }
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  // As `image_scale` in src/shot.rs: the DPR when the image is the clip at it, rounded.
  const scale = Math.abs(size - side * dpr) <= 1 ? dpr : size / side;
  return { x, y, scale, width: size, height: size, rgb: btoa(binary) };
}
