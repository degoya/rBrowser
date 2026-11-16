import { describe, expect, it } from 'vitest';
import { type Card, imageNote, imageSize, type ImageInfo, isPreview, otherLayout, reasonText, safeDataUri, shownLayouts, sourceLink } from './model';

const STRINGS: Record<string, string> = {
  'panel.no_image': 'No share image',
  'panel.image_failed': 'Failed ({reason}).',
  'panel.image_large': 'Too large ({bytes} bytes).',
  'panel.image_size': '{width} x {height} {format}',
  'panel.reasons.below': 'The share image ({width} x {height}) is below {min_width} x {min_height}.',
};

/** A stand-in for the panel's `t`: the text with its `{name}`s filled. */
const t = (key: string, params: Record<string, unknown> = {}): string =>
  (STRINGS[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? ''));

function image(patch: Partial<ImageInfo>): ImageInfo {
  return { url: 'https://a.test/s.png', status: 200, error: null, format: 'image/png', width: 1200, height: 630, bytes: 10, dataUri: null, tooLarge: false, ...patch };
}

function card(patch: Partial<Card>): Card {
  return {
    id: 'facebook',
    name: 'Facebook',
    layout: 'small',
    alternate: 'large',
    unsure: false,
    reason: { code: 'below', params: { width: '400', height: '400', min_width: '600', min_height: '315' } },
    confidence: 'official',
    source: 'https://developers.facebook.com/docs/sharing/webmasters/images/',
    title: null,
    description: null,
    image: null,
    imageSource: null,
    ...patch,
  };
}

describe('the panel model', () => {
  it('accepts only the shape of the tool answer, every card with its variant', () => {
    expect(isPreview({ url: 'u', domain: 'd', cards: [], images: [] })).toBe(true);
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({})], images: [] })).toBe(true);
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({ layout: 'huge' as 'large' })], images: [] })).toBe(false);
    expect(isPreview({ url: 'u', domain: 'd', cards: [{ id: 'x', name: 'X', layout: 'large' }], images: [] })).toBe(false);
    // Review of RB-080-85 (3): every field the panel builds a text key from is checked.
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({ confidence: 'sure' as 'official' })], images: [] })).toBe(false);
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({ unsure: 'no' as unknown as boolean })], images: [] })).toBe(false);
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({ reason: { code: 'below', params: null as unknown as Record<string, string> } })], images: [] })).toBe(false);
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({ reason: { code: 'below', params: { width: 4 as unknown as string } } })], images: [] })).toBe(false);
    expect(isPreview({ url: 'u', domain: 'd', cards: [card({ reason: { code: 'a.b', params: {} } })], images: [] })).toBe(false);
    expect(isPreview({ url: 'u', cards: [] })).toBe(false);
    expect(isPreview(null)).toBe(false);
  });

  it('shows the chosen variant and keeps the other behind the toggle; an unsure rule shows both', () => {
    expect(shownLayouts(card({}))).toEqual(['small']);
    expect(otherLayout(card({}))).toBe('large');
    const unsure = card({ layout: 'large', alternate: 'small', unsure: true });
    expect(shownLayouts(unsure)).toEqual(['large', 'small']);
    expect(otherLayout(unsure)).toBeNull();
    const text = card({ layout: 'text', alternate: null });
    expect(shownLayouts(text)).toEqual(['text']);
    expect(otherLayout(text)).toBeNull();
  });

  it('says the reason in words', () => {
    expect(reasonText(t, card({}))).toBe('The share image (400 x 400) is below 600 x 315.');
  });

  it('lets only a raster data: URI become an image source', () => {
    expect(safeDataUri(image({ dataUri: 'data:image/png;base64,iVBORw0KGgo=' }))).toBe('data:image/png;base64,iVBORw0KGgo=');
    expect(safeDataUri(image({ dataUri: 'data:image/svg+xml;base64,PHN2Zz4=' }))).toBeNull();
    expect(safeDataUri(image({ dataUri: 'https://a.test/s.png' }))).toBeNull();
    expect(safeDataUri(image({ dataUri: 'data:image/png;base64,"><script>' }))).toBeNull();
    expect(safeDataUri(undefined)).toBeNull();
  });

  it('says why an image is not shown, and its size', () => {
    expect(imageNote(t, undefined)).toBe('No share image');
    expect(imageNote(t, image({ status: 404 }))).toBe('Failed (404).');
    // RB-080-85: a failure is a sentence, never its code.
    const say = (code: string): string => (code === 'plugins.error.http_url_denied' ? 'The address is not allowed.' : 'The plugin could not do that.');
    expect(imageNote(t, image({ status: null, error: 'plugins.error.http_url_denied' }), say)).toBe('Failed (The address is not allowed.).');
    expect(imageNote(t, image({ tooLarge: true, bytes: 90000 }))).toBe('Too large (90000 bytes).');
    expect(imageNote(t, image({}))).toBe('');
    expect(imageSize(t, image({}))).toBe('1200 x 630 image/png');
    expect(imageSize(t, image({ width: null }))).toBe('');
  });

  it('opens only an https: source', () => {
    expect(sourceLink('https://docs.discord.com/developers/link-previews/overview')).toBe('https://docs.discord.com/developers/link-previews/overview');
    expect(sourceLink('javascript:alert(1)')).toBeNull();
    expect(sourceLink('http://a.test/')).toBeNull();
    expect(sourceLink(null)).toBeNull();
  });
});
