// The colour picker panel's pure part (RB-080-86).
import { describe, expect, it } from 'vitest';
import { asMeasured, asRating, failureKey, order } from './panel/model';

describe('the panel model', () => {
  it('puts the default format first and keeps the others in their order', () => {
    expect(order('oklch')).toEqual(['oklch', 'hex', 'rgb', 'hsl']);
    expect(order(undefined)).toEqual(['hex', 'rgb', 'hsl', 'oklch']);
    expect(order('cmyk')).toEqual(['hex', 'rgb', 'hsl', 'oklch']);
  });

  it('takes only a complete contrast answer and a measure with views', () => {
    expect(asRating({ ratio: 2 })).toBeNull();
    expect(asRating(null)).toBeNull();
    const formats = { hex: '#000000', rgb: '', hsl: '', oklch: '' };
    expect(asRating({ ratio: 2, foreground: formats, background: formats })?.ratio).toBe(2);
    expect(asMeasured({ selector: 'p' })).toBeNull();
    expect(asMeasured({ selector: 'p', views: [] })?.selector).toBe('p');
  });

  it('finds the text of a failure code, full or below the prefix, else none', () => {
    const strings = { 'plugin.x.errors.gone': 'Gone.', 'plugins.error.busy': 'Busy.' };
    expect(failureKey('plugins.error.busy', strings, 'plugin.x.')).toBe('plugins.error.busy');
    expect(failureKey('errors.gone', strings, 'plugin.x.')).toBe('plugin.x.errors.gone');
    expect(failureKey('plugins.error.host_failed', strings, 'plugin.x.')).toBeNull();
  });
});
