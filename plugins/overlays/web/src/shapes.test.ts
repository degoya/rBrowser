import { describe, expect, it } from 'vitest';
import { Layer } from './layer';
import { MARKER_SIZE, shapeStyle } from './shapes';

describe('the shapes of the overlay layer', () => {
  it('centre a numbered marker on its point', () => {
    const style = shapeStyle({ kind: 'marker', x: 100, y: 50, text: '12', color: '#1d4ed8' });
    expect(style.left).toBe(`${100 - MARKER_SIZE / 2}px`);
    expect(style.top).toBe(`${50 - MARKER_SIZE / 2}px`);
    expect(style['border-radius']).toBe(`${MARKER_SIZE / 2}px`);
    expect(style.color).toBe('#ffffff');
  });

  it('draw a line as a bar from its start, turned towards its end', () => {
    const style = shapeStyle({ kind: 'line', x1: 0, y1: 0, x2: 30, y2: 40, color: '#c2410c' });
    expect(style.width).toBe('50px');
    expect(style.transform).toBe(`rotate(${Math.atan2(40, 30)}rad)`);
    expect(style['transform-origin']).toBe('0 50%');
  });

  it('render markers with their text and drop the ones no longer drawn', () => {
    const layer = new Layer(document);
    layer.renderShapes('tab-order', [
      { kind: 'marker', x: 1, y: 1, text: '1', color: '#1d4ed8' },
      { kind: 'marker', x: 9, y: 9, text: '2', color: '#1d4ed8' },
    ]);
    expect(layer.count('tab-order')).toBe(2);
    layer.renderShapes('tab-order', [{ kind: 'line', x1: 0, y1: 0, x2: 1, y2: 1, color: '#1d4ed8' }]);
    expect(layer.count('tab-order')).toBe(1);
    layer.destroy();
    expect(document.querySelector('rbrowser-overlays')).toBeNull();
  });
});
