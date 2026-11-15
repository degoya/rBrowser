import { describe, expect, it } from 'vitest';
import * as editor from './panel/editor';
import type { ShotOptions } from './panel/types';

const OPTIONS: ShotOptions = { color: '#e53935', width: 4, size: 20, mode: 'blur', block: 16, fill: false };

describe('the editor model', () => {
  it('has nine tools, each with its key inside the panel', () => {
    expect(editor.TOOLS.map(([tool]) => tool)).toEqual(['rect', 'arrow', 'line', 'freehand', 'text', 'marker', 'highlight', 'redact', 'crop']);
    expect(editor.toolForKey('R')).toBe('rect');
    expect(editor.toolForKey('b')).toBe('redact');
    expect(editor.toolForKey('c')).toBe('crop');
    expect(editor.toolForKey('x')).toBeNull();
  });

  it('makes an annotation of every tool from a drag, rectangles normalised', () => {
    const drag = [
      [30, 40],
      [10, 15],
    ];
    expect(editor.build('rect', drag, OPTIONS, [])).toEqual({ type: 'rect', x: 10, y: 15, w: 20, h: 25, color: '#e53935', width: 4, fill: false });
    expect(editor.build('highlight', drag, OPTIONS, [])).toEqual({ type: 'highlight', x: 10, y: 15, w: 20, h: 25, color: '#e53935' });
    expect(editor.build('redact', drag, OPTIONS, [])).toEqual({ type: 'redact', x: 10, y: 15, w: 20, h: 25, mode: 'blur', block: 16 });
    expect(editor.build('crop', drag, OPTIONS, [])).toEqual({ type: 'crop', x: 10, y: 15, w: 20, h: 25 });
    expect(editor.build('arrow', drag, OPTIONS, [])).toEqual({ type: 'arrow', x1: 30, y1: 40, x2: 10, y2: 15, color: '#e53935', width: 4 });
    expect(editor.build('line', drag, OPTIONS, [])?.type).toBe('line');
    expect(editor.build('freehand', [[0, 0], [0.5, 0.5], [5, 5], [5.04, 5.04]], OPTIONS, [])).toEqual({
      type: 'freehand',
      points: [
        [0, 0],
        [5, 5],
      ],
      color: '#e53935',
      width: 4,
    });
    expect(editor.build('text', [[3, 4]], OPTIONS, [], '  Login\tfails  ')).toEqual({ type: 'text', x: 3, y: 4, text: 'Login fails', color: '#e53935', size: 20 });
    expect(editor.build('marker', [[3, 4]], OPTIONS, [{ type: 'marker', number: 4 }])).toEqual({ type: 'marker', x: 3, y: 4, number: 5, color: '#e53935', size: 20 });
  });

  it('makes nothing of a click where a drag is needed or of empty text', () => {
    for (const tool of ['rect', 'arrow', 'line', 'highlight', 'redact', 'crop']) {
      expect(editor.build(tool, [[5, 5], [5.2, 5.2]], OPTIONS, []), tool).toBeNull();
    }
    expect(editor.build('text', [[1, 1]], OPTIONS, [], '   ')).toBeNull();
    expect(editor.build('text', [[1, 1]], OPTIONS, [], 'x'.repeat(300))?.text).toHaveLength(editor.MAX_TEXT);
  });

  it('keeps one crop: a new one replaces the old', () => {
    const one = editor.add([], { type: 'crop', x: 0, y: 0, w: 5, h: 5 });
    const two = editor.add([...one, { type: 'marker', number: 1 }], { type: 'crop', x: 1, y: 1, w: 2, h: 2 });
    expect(two).toEqual([{ type: 'marker', number: 1 }, { type: 'crop', x: 1, y: 1, w: 2, h: 2 }]);
  });

  it('undoes and redoes more than fifty steps', () => {
    const history = editor.history();
    expect(editor.HISTORY).toBeGreaterThanOrEqual(50);
    for (let step = 1; step <= 60; step += 1) history.push([...history.items, { type: 'marker', number: step }]);
    for (let step = 0; step < 60; step += 1) history.undo();
    expect(history.items).toEqual([]);
    expect(history.canUndo).toBe(false);
    for (let step = 0; step < 60; step += 1) history.redo();
    expect(history.items).toHaveLength(60);
    history.undo();
    history.push([]);
    expect(history.canRedo).toBe(false);
  });

  it('forgets the oldest steps past its limit and starts again on reset', () => {
    const history = editor.history(3);
    for (let step = 1; step <= 5; step += 1) history.push([{ type: 'marker', number: step }]);
    history.undo();
    history.undo();
    history.undo();
    expect(history.items).toEqual([{ type: 'marker', number: 2 }]);
    expect(history.canUndo).toBe(false);
    history.reset([{ type: 'crop' }]);
    expect(history.items).toEqual([{ type: 'crop' }]);
    expect(history.canRedo).toBe(false);
  });

  it('knows when a save would pass a panel message', () => {
    expect(editor.fits({ image: 'a.png', annotations: [] })).toBe(true);
    const huge = Array.from({ length: 8000 }, (_, i) => [i, i]);
    expect(editor.fits({ image: 'a.png', annotations: [{ type: 'freehand', points: huge }] })).toBe(false);
  });
});
