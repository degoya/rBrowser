import { describe, expect, it } from 'vitest';
import type { Answer, FaceEntry, Family } from './model';
import { cssFamily, FontPreviews, mainFace, sourceView, type PreviewPart } from './preview';

const face = (src: string | null, weight = '400', status = [{ view: 1, state: 'loaded' }]): FaceEntry => ({
  weight, style: 'normal', display: 'swap', unicodeRange: '', src, format: 'woff2', kb: 1, preloaded: false, status,
});

const family = (name: string, faces: FaceEntry[], webFont = true): Family => ({
  family: name, provider: webFont ? 'own' : 'system', host: '', webFont, formats: [], variable: false, faces, fileKb: null, usage: [], rendered: [],
});

const ANSWER: Answer = {
  views: [
    { id: 1, label: 'md', width: 768, engine: 'chromium' },
    { id: 2, label: 'mobile', width: 375, engine: 'firefox', error: 'unsupported' },
  ],
  families: [],
  roles: [],
};

const flush = async (): Promise<void> => {
  for (let i = 0; i < 10; i += 1) await new Promise((resolve) => setTimeout(resolve, 0));
};

function previews(parts: Record<string, PreviewPart[]>) {
  const reads: [number, string, number][] = [];
  const loaded: { family: string; bytes: number[] | string; weight: string }[] = [];
  const list = new FontPreviews({
    read: async (view, src, offset) => {
      reads.push([view, src, offset]);
      const queue = parts[src] ?? [];
      return queue.shift() ?? { src, error: 'unreadable' };
    },
    load: async (name, source, descriptors) => {
      if (typeof source !== 'string' && source.byteLength === 0) throw new Error('empty');
      loaded.push({ family: name, bytes: typeof source === 'string' ? source : [...new Uint8Array(source)], weight: descriptors.weight });
    },
  });
  return { list, reads, loaded };
}

describe('the font previews', () => {
  it('shows an installed or generic family by its name', () => {
    const { list } = previews({});
    expect(list.state(family('Arial', [], false), undefined)).toEqual({ kind: 'ready', css: '"Arial"' });
    expect(list.state(family('ui-monospace', [], false), undefined)).toEqual({ kind: 'ready', css: 'ui-monospace' });
    expect(cssFamily('My "Font"')).toBe('"My \\"Font\\""');
  });

  it('reads a web font part by part from a view that loaded it, and loads it as a FontFace', async () => {
    const src = 'https://x.test/a.woff2';
    const { list, reads, loaded } = previews({
      [src]: [
        { src, offset: 0, total: 5, data: btoa('\x00\x01\x02'), done: false },
        { src, offset: 3, total: 5, data: btoa('\x03\x04'), done: true },
      ],
    });
    const one = face(src);
    const fam = family('Vision', [one]);
    list.request(ANSWER, [fam]);
    expect(list.state(fam, one)).toEqual({ kind: 'loading' });
    await flush();
    expect(reads).toEqual([[1, src, 0], [1, src, 3]]);
    expect(loaded).toEqual([{ family: 'rb-preview-1', bytes: [0, 1, 2, 3, 4], weight: '400' }]);
    expect(list.state(fam, one)).toEqual({ kind: 'ready', css: '"rb-preview-1"' });
    // Asked again: read once.
    list.request(ANSWER, [fam]);
    await flush();
    expect(reads).toHaveLength(2);
  });

  it('says why a face has no preview', async () => {
    const big = 'https://x.test/big.woff2';
    const { list } = previews({ [big]: [{ src: big, error: 'too_large', maxKb: 1024 }] });
    const faces = {
      big: face(big),
      none: face(null),
      relative: face('fonts/a.woff2'),
      firefox: face('https://x.test/f.woff2', '400', [{ view: 2, state: 'loaded' }]),
      unreadable: face('https://x.test/u.woff2'),
    };
    const fam = family('Vision', Object.values(faces));
    list.request(ANSWER, [fam]);
    await flush();
    expect(list.state(fam, faces.big)).toEqual({ kind: 'missing', reason: 'too_large', kb: 1024 });
    expect(list.state(fam, faces.none)).toEqual({ kind: 'missing', reason: 'no_file' });
    expect(list.state(fam, faces.relative)).toEqual({ kind: 'missing', reason: 'no_file' });
    expect(list.state(fam, faces.firefox)).toEqual({ kind: 'missing', reason: 'not_loaded' });
    expect(list.state(fam, faces.unreadable)).toEqual({ kind: 'missing', reason: 'unreadable' });
    expect(list.state(family('Empty', []), undefined)).toEqual({ kind: 'missing', reason: 'no_file' });
  });

  it('says when the browser cannot read the file', async () => {
    const src = 'https://x.test/broken.woff2';
    const { list } = previews({ [src]: [{ src, offset: 0, total: 0, data: '', done: true }] });
    const one = face(src);
    const fam = family('Broken', [one]);
    list.request(ANSWER, [fam]);
    await flush();
    expect(list.state(fam, one)).toEqual({ kind: 'missing', reason: 'failed' });
  });

  it('loads an embedded font from its data address without the tool', async () => {
    const src = 'data:font/woff2;base64,AAAA';
    const { list, reads, loaded } = previews({});
    const one = face(src);
    list.request(ANSWER, [family('Inline', [one])]);
    await flush();
    expect(reads).toEqual([]);
    expect(loaded[0]?.bytes).toBe(`url("${src}")`);
  });

  it('picks the view and the face of the one-line preview', () => {
    expect(sourceView(ANSWER, face('x', '400', [{ view: 2, state: 'loaded' }, { view: 1, state: 'loaded' }]))).toBe(1);
    expect(sourceView(ANSWER, face('x', '400', [{ view: 1, state: 'error' }]))).toBeNull();
    const bold = face('b', '700');
    const regular = face('r', '400');
    expect(mainFace(family('F', [bold, regular]))).toBe(regular);
    expect(mainFace(family('F', [bold]))).toBe(bold);
  });
});
