import { describe, expect, it } from 'vitest';
import { boundsOf, evaluate, parseCondition, parsePrelude, type Sizes } from './query';

const units = { em: 16, rem: 16, vw: 1280, vh: 800 };
const at = (inline: number, block = 300): Sizes => ({ inline, block, ...units });

describe('the prelude of an @container rule', () => {
  it('splits an optional name from the query', () => {
    expect(parsePrelude('card (min-width: 500px)')).toEqual({ name: 'card', query: '(min-width: 500px)' });
    expect(parsePrelude('(width > 400px)')).toEqual({ name: '', query: '(width > 400px)' });
    expect(parsePrelude('  sidebar   not (width < 20rem) ')).toEqual({ name: 'sidebar', query: 'not (width < 20rem)' });
    expect(parsePrelude('not (width < 20rem)')).toEqual({ name: '', query: 'not (width < 20rem)' });
  });
});

describe('a container condition', () => {
  it('evaluates min-, max- and plain size features', () => {
    const min = parseCondition('(min-width: 500px)');
    expect(evaluate(min, at(500))).toBe(true);
    expect(evaluate(min, at(400))).toBe(false);
    expect(evaluate(parseCondition('(max-inline-size: 30em)'), at(480))).toBe(true);
    expect(evaluate(parseCondition('(max-inline-size: 30em)'), at(481))).toBe(false);
    expect(evaluate(parseCondition('(min-height: 200px)'), at(10, 250))).toBe(true);
    expect(evaluate(parseCondition('(width: 300px)'), at(300))).toBe(true);
  });

  it('evaluates the range syntax in both directions and as an interval', () => {
    expect(evaluate(parseCondition('(width > 400px)'), at(400))).toBe(false);
    expect(evaluate(parseCondition('(width >= 400px)'), at(400))).toBe(true);
    expect(evaluate(parseCondition('(400px < width)'), at(401))).toBe(true);
    const between = parseCondition('(400px <= inline-size < 800px)');
    expect(evaluate(between, at(400))).toBe(true);
    expect(evaluate(between, at(800))).toBe(false);
    expect(evaluate(between, at(399))).toBe(false);
  });

  it('combines with and, or and not', () => {
    const both = parseCondition('(min-width: 300px) and (max-width: 600px)');
    expect(evaluate(both, at(450))).toBe(true);
    expect(evaluate(both, at(700))).toBe(false);
    const either = parseCondition('(width < 200px) or (width > 900px)');
    expect(evaluate(either, at(100))).toBe(true);
    expect(evaluate(either, at(500))).toBe(false);
    expect(evaluate(parseCondition('not (width < 200px)'), at(500))).toBe(true);
    expect(evaluate(parseCondition('((width > 1px) and (height > 1px))'), at(5, 5))).toBe(true);
  });

  it('resolves rem, em and viewport units, and orientation and aspect-ratio', () => {
    expect(evaluate(parseCondition('(min-width: 40rem)'), { ...at(640), rem: 16 })).toBe(true);
    expect(evaluate(parseCondition('(min-width: 40em)'), { ...at(640), em: 20 })).toBe(false);
    expect(evaluate(parseCondition('(min-width: 50vw)'), at(640))).toBe(true);
    expect(evaluate(parseCondition('(orientation: landscape)'), at(640, 300))).toBe(true);
    expect(evaluate(parseCondition('(min-aspect-ratio: 16/9)'), at(1600, 900))).toBe(true);
    expect(evaluate(parseCondition('(aspect-ratio < 1)'), at(300, 600))).toBe(true);
  });

  it('answers unknown, never true, for what it cannot read', () => {
    expect(evaluate(parseCondition('style(--theme: dark)'), at(500))).toBeNull();
    expect(evaluate(parseCondition('(min-width: 3furlongs)'), at(500))).toBeNull();
    expect(evaluate(parseCondition('(min-width: 1px) and style(--x: 1)'), at(500))).toBeNull();
    expect(evaluate(parseCondition('(min-width: 1px) or style(--x: 1)'), at(500))).toBe(true);
    expect(evaluate(parseCondition('not style(--x: 1)'), at(500))).toBeNull();
    expect(evaluate(parseCondition('(width'), at(500))).toBeNull();
  });
});

describe('the bounds of a condition', () => {
  it('names each size threshold with its axis, operator and px', () => {
    expect(boundsOf(parseCondition('(min-width: 500px)'), units)).toEqual([
      { axis: 'inline', op: '>=', value: '500px', px: 500 },
    ]);
    expect(boundsOf(parseCondition('(400px <= width < 50rem)'), units)).toEqual([
      { axis: 'inline', op: '>=', value: '400px', px: 400 },
      { axis: 'inline', op: '<', value: '50rem', px: 800 },
    ]);
    expect(boundsOf(parseCondition('(max-block-size: 300px) or (orientation: portrait)'), units)).toEqual([
      { axis: 'block', op: '<=', value: '300px', px: 300 },
    ]);
  });

  it('says which axes a condition reads', () => {
    expect(boundsOf(parseCondition('style(--x: 1)'), units)).toEqual([]);
  });
});
