import { describe, expect, it } from 'vitest';
import { allowsAlpha, opacityOf, parseColor, withOpacity, withRgb } from './colorValue';

describe('a colour value', () => {
  it('reads #rrggbb and #rrggbbaa, nothing else', () => {
    expect(parseColor('#336699')).toEqual({ rgb: '#336699', alpha: 255 });
    expect(parseColor('#33669980')).toEqual({ rgb: '#336699', alpha: 128 });
    expect(parseColor('#AABBCC')).toEqual({ rgb: '#aabbcc', alpha: 255 });
    for (const bad of ['', '336699', '#369', '#3366991', 'red', '#33669g', null, 7]) {
      expect(parseColor(bad), String(bad)).toBeNull();
    }
  });

  it('takes the hue the picker chose and keeps the opacity', () => {
    expect(withRgb('#33669980', '#FF0000', true)).toBe('#ff000080');
    expect(withRgb('#336699', '#FF0000', true)).toBe('#ff0000');
    // Where the setting has no opacity, none is written.
    expect(withRgb('#33669980', '#FF0000', false)).toBe('#ff0000');
    // A value that is none yet starts opaque.
    expect(withRgb('', '#00ff00', true)).toBe('#00ff00');
    expect(withRgb('#336699', 'not a colour', true)).toBe('#336699');
  });

  it('sets the opacity in per cent; fully opaque is six digits', () => {
    expect(opacityOf('#33669980')).toBe(50);
    expect(opacityOf('#336699')).toBe(100);
    expect(opacityOf('nonsense')).toBe(100);
    expect(withOpacity('#336699', 50)).toBe('#33669980');
    expect(withOpacity('#33669980', 100)).toBe('#336699');
    expect(withOpacity('#336699', 0)).toBe('#33669900');
    expect(withOpacity('#336699', 250)).toBe('#336699');
    expect(withOpacity('', 50)).toBe('#00000080');
  });

  it('offers an opacity where the schema allows eight digits', () => {
    expect(allowsAlpha({ type: 'string', pattern: '^#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$' })).toBe(true);
    expect(allowsAlpha({ type: 'string', pattern: '^#[0-9A-Fa-f]{6}$' })).toBe(false);
    expect(allowsAlpha({ type: 'string' })).toBe(false);
    expect(allowsAlpha({ type: 'string', pattern: '([' })).toBe(false);
  });
});
