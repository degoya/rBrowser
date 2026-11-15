import { describe, expect, it } from 'vitest';
import { AUTO, designChoices, isMissing, nameOf, nudged, type Design } from './panel/model';

const t = (key: string, params: Record<string, unknown> = {}): string => `${key}${JSON.stringify(params)}`;
const DESKTOP: Design = { name: 'desktop.png', path: 'designs/desktop.png', width: 1440, height: 900 };
const BROKEN: Design = { name: 'broken.png', path: 'designs/broken.png', problem: { code: 'x' } };

describe('the image-overlay panel model', () => {
  it('names a design below the designs folder', () => {
    expect([nameOf('designs/a.png'), nameOf('.rbrowser/designs/b/c.png'), nameOf('x.png')]).toEqual(['a.png', 'b/c.png', 'x.png']);
  });

  it('offers Nearest width first, then each design by name and size, unusable and missing ones marked', () => {
    const choices = designChoices(t, ['broken.png', 'desktop.png', 'new.png'], [BROKEN, DESKTOP], 'designs/old.png');
    expect(choices.map((choice) => choice.value)).toEqual([AUTO, 'designs/broken.png', 'designs/desktop.png', 'designs/new.png', 'designs/old.png']);
    expect(choices.map((choice) => choice.label)).toEqual([
      'panel.design_auto{}',
      'panel.option_unusable{"name":"broken.png"}',
      'panel.option_design{"name":"desktop.png","width":1440,"height":900}',
      // Not measured yet: the name alone.
      'new.png',
      'panel.option_missing{"name":"old.png"}',
    ]);
    expect(choices.filter((choice) => choice.missing).map((choice) => choice.value)).toEqual(['designs/old.png']);
    expect(designChoices(t, ['desktop.png'], [DESKTOP], 'designs/desktop.png')).toHaveLength(2);
  });

  it('says whether a chosen design left the folder', () => {
    expect([isMissing(['a.png'], 'designs/a.png'), isMissing(['a.png'], 'designs/b.png'), isMissing([], undefined)]).toEqual([false, true, false]);
  });

  it('moves the offset by arrow keys, 1 px or 10 px with Shift', () => {
    expect(nudged({ x: 0, y: 0 }, 'ArrowRight', false)).toEqual({ x: 1, y: 0 });
    expect(nudged({ x: 1, y: 0 }, 'ArrowUp', true)).toEqual({ x: 1, y: -10 });
    expect(nudged({ x: 0, y: 0 }, 'Enter', false)).toBeNull();
  });
});
