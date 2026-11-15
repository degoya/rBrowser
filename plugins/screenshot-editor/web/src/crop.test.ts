import { describe, expect, it } from 'vitest';
import * as crop from './panel/crop';

// RB-080-63: the crop frame's handles - which one a press takes, and the frame after a drag.
const FRAME = { type: 'crop', x: 10, y: 20, w: 40, h: 30 };

describe('the crop frame', () => {
  it('finds a handle near a press, the inside as move, else nothing', () => {
    expect(crop.hit(FRAME, [10, 20], 4)).toBe('nw');
    expect(crop.hit(FRAME, [52, 51], 4)).toBe('se');
    expect(crop.hit(FRAME, [30, 21], 4)).toBe('n');
    expect(crop.hit(FRAME, [49, 35], 4)).toBe('e');
    expect(crop.hit(FRAME, [30, 35], 4)).toBe('move');
    expect(crop.hit(FRAME, [80, 80], 4)).toBeNull();
    expect(crop.HANDLES).toHaveLength(8);
  });

  it('resizes by a handle, inside the image, and turns over past the opposite edge', () => {
    expect(crop.drag(FRAME, 'se', [50, 50], [60, 55], 100, 80)).toEqual({ type: 'crop', x: 10, y: 20, w: 50, h: 35 });
    expect(crop.drag(FRAME, 'e', [50, 35], [500, 90], 100, 80)).toEqual({ type: 'crop', x: 10, y: 20, w: 90, h: 30 });
    expect(crop.drag(FRAME, 'nw', [10, 20], [70, 60], 100, 80)).toEqual({ type: 'crop', x: 50, y: 50, w: 20, h: 10 });
    // A frame that would be empty stays as it was.
    expect(crop.drag(FRAME, 'w', [10, 35], [50, 35], 100, 80)).toBe(FRAME);
  });

  it('moves the whole frame and keeps it in the image', () => {
    expect(crop.drag(FRAME, 'move', [30, 30], [35, 32], 100, 80)).toEqual({ type: 'crop', x: 15, y: 22, w: 40, h: 30 });
    expect(crop.drag(FRAME, 'move', [30, 30], [300, -50], 100, 80)).toEqual({ type: 'crop', x: 60, y: 0, w: 40, h: 30 });
  });

  it('names the size the export will have, in whole pixels', () => {
    expect(crop.size({ type: 'crop', x: 0.5, y: 0, w: 10, h: 5.2 }, 100, 80)).toEqual({ width: 11, height: 6 });
    expect(crop.size({ type: 'crop', x: 90, y: 0, w: 40, h: 10 }, 100, 80)).toEqual({ width: 10, height: 10 });
  });
});
