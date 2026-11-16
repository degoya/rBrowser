// The picture of the grid in the analyser's wizard (RB-090-43): the cells where `gridPreview`
// puts them (the plugin's grid, `plugins/breakpoint-analyzer/src/apply.rs`), every view drawn
// at one common scale so the sizes compare, each column as wide as its widest view so the boxes
// of a row never overlap, the whole as wide as the room it has. The label and size go under
// each box, so a narrow view keeps a readable caption; a column is never narrower than one.

import type { GridPreview, PreviewCell } from './wizard';

export const PREVIEW = {
  /** Between columns and rows, px. */
  gap: 12,
  /** The tallest box, px: six views of a laptop screen stay one glance high. */
  maxBoxHeight: 160,
  /** The narrowest column, px: room for a label and `1920 x 1200` under the box. */
  minTrack: 72,
  /** The width before the figure was measured (a test, the first frame), px. */
  fallbackWidth: 560,
} as const;

export interface PlacedCell extends PreviewCell {
  /** The view's box, px. */
  boxWidth: number;
  boxHeight: number;
}

export interface PreviewGeometry {
  /** px per CSS px of the views, the same for every box. */
  scale: number;
  columns: number;
  rows: number;
  /** The width of each column, px. */
  trackWidths: number[];
  /** Columns and gaps together, px. */
  width: number;
  cells: PlacedCell[];
}

/** Lays `grid` out in `available` px: the common scale is the largest at which the columns
 *  fit the width and the tallest view stays [`PREVIEW.maxBoxHeight`]; a column its views would
 *  make narrower than [`PREVIEW.minTrack`] keeps that width and the others share the rest. */
export function previewGeometry(grid: GridPreview, available: number): PreviewGeometry {
  if (grid.cells.length === 0) {
    return { scale: 0, columns: grid.columns, rows: grid.rows, trackWidths: [], width: 0, cells: [] };
  }
  const widest = Array.from({ length: grid.columns }, (_, index) =>
    Math.max(0, ...grid.cells.filter((cell) => cell.col === index + 1).map((cell) => cell.width)),
  );
  const tallest = Math.max(1, ...grid.cells.map((cell) => cell.height));
  const room = Math.max(0, available - PREVIEW.gap * (grid.columns - 1));
  const heightScale = PREVIEW.maxBoxHeight / tallest;

  // Columns pinned to the minimum leave the rest of the room to the others; pinning one can
  // shrink the scale and pin another, so this settles within `columns` rounds.
  const pinned = new Set<number>();
  let scale = heightScale;
  for (let round = 0; round <= grid.columns; round += 1) {
    const free = widest.reduce((sum, width, index) => (pinned.has(index) ? sum : sum + width), 0);
    if (free === 0) {
      // Every column at the minimum: the last scale fits each box into its column.
      break;
    }
    const left = room - pinned.size * PREVIEW.minTrack;
    scale = Math.min(heightScale, Math.max(0, left) / free);
    const next = widest.findIndex((width, index) => !pinned.has(index) && width * scale < PREVIEW.minTrack);
    if (next < 0) {
      break;
    }
    pinned.add(next);
  }

  const trackWidths = widest.map((width) => Math.max(PREVIEW.minTrack, Math.floor(width * scale)));
  const cells = grid.cells.map((cell) => ({
    ...cell,
    boxWidth: Math.floor(cell.width * scale),
    boxHeight: Math.floor(cell.height * scale),
  }));
  const width = trackWidths.reduce((sum, track) => sum + track, 0) + PREVIEW.gap * (grid.columns - 1);
  return { scale, columns: grid.columns, rows: grid.rows, trackWidths, width, cells };
}
