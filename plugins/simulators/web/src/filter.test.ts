import { describe, expect, it } from 'vitest';
import { cssFilter, readSpec } from './filter';

const PROTANOPIA = [
  0.1523, 1.0526, -0.2049, 0, 0, 0.1145, 0.7863, 0.0992, 0, 0, -0.0039, -0.0481, 1.052, 0, 0, 0, 0, 0, 1, 0,
];

describe('readSpec', () => {
  it('takes what the component sends', () => {
    const spec = readSpec({
      filter: [
        { op: 'matrix', values: PROTANOPIA },
        { op: 'blur', px: 2 },
        { op: 'contrast', amount: 0.6 },
        { op: 'brightness', amount: 1.4 },
        { op: 'saturate', amount: 0.5 },
      ],
      overlay: { kind: 'glare', opacity: 0.35 },
    });
    expect(spec?.filter).toHaveLength(5);
    expect(spec?.overlay).toEqual({ kind: 'glare', opacity: 0.35 });
  });

  it('reads null and no object as nothing to draw', () => {
    expect(readSpec(null)).toBeNull();
    expect(readSpec('protanopia')).toBeNull();
    expect(readSpec({ filter: [], overlay: null })).toBeNull();
  });

  it('refuses anything it could not draw safely', () => {
    const refused = [
      { filter: [{ op: 'url', value: 'https://example.com/x.svg#f' }], overlay: null },
      { filter: [{ op: 'matrix', values: PROTANOPIA.slice(0, 19) }], overlay: null },
      { filter: [{ op: 'matrix', values: [...PROTANOPIA.slice(0, 19), Number.NaN] }], overlay: null },
      { filter: [{ op: 'blur', px: '2px); background: url(x' }], overlay: null },
      { filter: [{ op: 'blur', px: 1e9 }], overlay: null },
      { filter: Array.from({ length: 9 }, () => ({ op: 'blur', px: 1 })), overlay: null },
      { filter: [], overlay: { kind: 'image', url: 'x' } },
      { filter: [], overlay: { kind: 'tunnel', clear: 0.2, fade: 'x', opacity: 1 } },
      { filter: 'blur(2px)', overlay: null },
    ];
    for (const spec of refused) expect(readSpec(spec), JSON.stringify(spec)).toBeNull();
  });
});

describe('cssFilter', () => {
  it('names each matrix by its SVG filter and the rest as CSS functions, in order', () => {
    const spec = readSpec({
      filter: [
        { op: 'matrix', values: PROTANOPIA },
        { op: 'blur', px: 2 },
        { op: 'contrast', amount: 0.6 },
      ],
      overlay: null,
    });
    expect(spec).not.toBeNull();
    expect(cssFilter(spec!.filter, (index) => `m${index}`)).toBe('url("#m0") blur(2px) contrast(0.6)');
  });

  it('is none for an overlay alone', () => {
    const spec = readSpec({ filter: [], overlay: { kind: 'tunnel', clear: 0.24, fade: 0.25, opacity: 0.95 } });
    expect(cssFilter(spec!.filter, (index) => `m${index}`)).toBe('');
  });
});
