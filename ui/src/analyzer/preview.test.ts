import { describe, expect, it } from 'vitest';
import { PREVIEW, previewGeometry } from './preview';
import { gridPreview, type ProposedView } from './wizard';

/** The six views of the owner's screen (RB-090-43). */
const SIX: ProposedView[] = [
  ['mobile', 375, 667],
  ['sm', 640, 1138],
  ['lg', 1024, 640],
  ['xl', 1280, 800],
  ['2xl', 1536, 960],
  ['desktop', 1920, 1200],
].map(([id, width, height]) => ({ id: String(id), label: String(id), width: Number(width), height: Number(height), isNew: true }));

function trackSum(widths: number[]): number {
  return widths.reduce((sum, width) => sum + width, 0) + PREVIEW.gap * (widths.length - 1);
}

describe('the preview geometry', () => {
  it('draws every view with one common scale', () => {
    const geometry = previewGeometry(gridPreview(SIX), 600);
    for (const cell of geometry.cells) {
      expect(cell.boxWidth).toBe(Math.floor(cell.width * geometry.scale));
      expect(cell.boxHeight).toBe(Math.floor(cell.height * geometry.scale));
    }
    const byId = new Map(geometry.cells.map((cell) => [cell.id, cell]));
    // The sizes compare: desktop is wider than 2xl, sm taller than mobile.
    expect(byId.get('desktop')!.boxWidth).toBeGreaterThan(byId.get('2xl')!.boxWidth);
    expect(byId.get('sm')!.boxHeight).toBeGreaterThan(byId.get('mobile')!.boxHeight);
    expect(geometry.scale).toBeGreaterThan(0);
  });

  it('keeps the arrangement of the grid that is written: four columns, then the next row', () => {
    const geometry = previewGeometry(gridPreview(SIX), 600);
    expect([geometry.columns, geometry.rows]).toEqual([4, 2]);
    expect(geometry.cells.map((cell) => [cell.id, cell.col, cell.row])).toEqual([
      ['mobile', 1, 1],
      ['sm', 2, 1],
      ['lg', 3, 1],
      ['xl', 4, 1],
      ['2xl', 1, 2],
      ['desktop', 2, 2],
    ]);
    // A column is as wide as its widest view, so a row's boxes never overlap.
    for (const cell of geometry.cells) {
      expect(geometry.trackWidths[cell.col - 1]).toBeGreaterThanOrEqual(cell.boxWidth);
    }
  });

  it('fits the available width and wraps into rows rather than overflowing', () => {
    for (const available of [600, 420, 900]) {
      const geometry = previewGeometry(gridPreview(SIX), available);
      expect(trackSum(geometry.trackWidths)).toBeLessThanOrEqual(available);
      expect(geometry.width).toBe(trackSum(geometry.trackWidths));
    }
  });

  it('keeps the boxes low and leaves room for the label under a narrow view', () => {
    const geometry = previewGeometry(gridPreview(SIX), 2000);
    expect(Math.max(...geometry.cells.map((cell) => cell.boxHeight))).toBeLessThanOrEqual(PREVIEW.maxBoxHeight);
    const narrow = previewGeometry(gridPreview(SIX), 340);
    expect(Math.min(...narrow.trackWidths)).toBeGreaterThanOrEqual(PREVIEW.minTrack);
  });

  it('puts the views appended to a grid in one new row', () => {
    const grid = gridPreview(SIX, { appended: true });
    expect([grid.columns, grid.rows]).toEqual([6, 1]);
    expect(grid.cells.map((cell) => cell.col)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(previewGeometry(grid, 600).width).toBeLessThanOrEqual(600);
  });

  it('draws nothing for no views', () => {
    const geometry = previewGeometry(gridPreview([]), 600);
    expect(geometry.cells).toEqual([]);
    expect(geometry.width).toBe(0);
  });
});
