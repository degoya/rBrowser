import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import fixture from '../../../../tests/fixtures/site/outline/index.html?raw';
import { collect, MAX_ENTRIES } from './collect';
import { run } from './collect-entry';
import type { Box } from './types';
import type { Env } from './visible';

/** happy-dom has no layout: every element a 10x10 box, the page 1280 px wide. */
const flat: Env = {
  rect: (): Box => [10, 10, 10, 10],
  pageWidth: () => 1280,
};

function load(html: string, width = 1280): void {
  (window as unknown as { happyDOM: { setViewport(v: { width: number; height: number }): void } }).happyDOM.setViewport({
    width,
    height: 800,
  });
  const body = /<body>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '';
  const style = /<style>([\s\S]*?)<\/style>/.exec(html)?.[1] ?? '';
  document.documentElement.setAttribute('lang', 'en');
  document.head.innerHTML = `<style>${style}</style>`;
  // The fixture's own script is no part of the outline.
  document.body.innerHTML = body.replace(/<script>[\s\S]*?<\/script>/g, '');
}

describe('collect against tests/fixtures/site/outline/', () => {
  beforeEach(() => load(fixture));
  afterEach(() => {
    document.head.innerHTML = '';
    document.body.innerHTML = '';
  });

  it('lists the headings with their levels and the h2 -> h4 jump', () => {
    const outline = collect({ sections: ['headings'] }, flat);
    expect(outline.headings.map((h) => `${h.level} ${h.text}`)).toEqual([
      '1 Outline fixture',
      '2 Introduction',
      '4 Details',
      '2 Contact',
      '2 Promotion',
    ]);
    expect(outline.headings[2]?.path).toBe('html > body > main > section:nth-of-type(1) > h4');
    expect(outline.links).toEqual([]);
  });

  it('finds the landmarks with role and name', () => {
    const outline = collect({ sections: ['landmarks'] }, flat);
    expect(outline.landmarks.map((l) => `${l.role}:${l.name}`)).toEqual([
      'banner:',
      'navigation:Main',
      'navigation:Mobile',
      'main:',
      'region:Intro',
      'region:Contact',
      'form:Contact form',
      'contentinfo:',
    ]);
  });

  it('marks the main navigation hidden in a narrow view and shown in a wide one', () => {
    const wide = collect({ sections: ['landmarks'] }, flat);
    load(fixture, 375);
    const narrow = collect({ sections: ['landmarks'] }, flat);
    const main = (outline: typeof wide) => outline.landmarks.find((l) => l.name === 'Main');
    expect(main(wide)?.visible).toBe(true);
    expect(main(narrow)?.visible).toBe(false);
    expect(main(narrow)?.hidden).toBe(true);
    const mobile = narrow.landmarks.find((l) => l.name === 'Mobile');
    expect(mobile?.hidden).toBe(true);
  });

  it('reads the links: internal, external, target=_blank without noopener, image-only', () => {
    const outline = collect({ sections: ['links'] }, flat);
    const old = outline.links.find((l) => l.text === 'Old page');
    expect(old?.href).toBe(`${location.origin}/outline/missing.html`);
    expect(old?.internal).toBe(true);
    const external = outline.links.find((l) => l.href === 'https://example.com/');
    expect(external?.internal).toBe(false);
    expect(external?.blankUnsafe).toBe(true);
    const gallery = outline.links.find((l) => l.href.endsWith('/outline/gallery.html'));
    expect(gallery?.text).toBe('');
  });

  it('reads the images: alt missing, alt empty, sizes', () => {
    const outline = collect({ sections: ['images'] }, flat);
    expect(outline.images.map((i) => i.alt)).toEqual(['A harbour at dusk', null, '', null]);
    expect(outline.images[0]?.width).toBe(320);
  });

  it('reads the form fields with their label source', () => {
    const outline = collect({ sections: ['forms'] }, flat);
    expect(outline.fields.map((f) => `${f.name}:${f.labelSource}:${f.label}`)).toEqual([
      'email:label:Email',
      'nickname:none:',
      'phone:aria-label:Phone',
    ]);
    expect(outline.fields[0]?.required).toBe(true);
    expect(outline.fields[0]?.autocomplete).toBe('email');
  });

  it('lists the focus order without hidden elements, positive tabindex first', () => {
    document.getElementById('nickname')?.setAttribute('tabindex', '1');
    const outline = collect({ sections: ['focus'] }, flat);
    expect(outline.focus[0]?.selector).toBe('#nickname');
    expect(outline.focus.some((f) => f.selector === '#burger')).toBe(false);
    expect(outline.focus.map((f) => f.name)).toContain('Old page');
  });

  it('counts the hits of a custom selector per view', () => {
    const custom = [{ label: 'CMS blocks', selector: '[data-cms-block]' }];
    const wide = collect({ sections: ['custom'], custom }, flat);
    expect(wide.custom[0]?.count).toBe(3);
    expect(wide.custom[0]?.hits.filter((hit) => hit.visible)).toHaveLength(3);
    load(fixture, 375);
    const narrow = collect({ sections: ['custom'], custom }, flat);
    expect(narrow.custom[0]?.hits.filter((hit) => hit.visible)).toHaveLength(2);
  });

  it('reports a selector that does not parse', () => {
    const outline = collect({ sections: ['custom'], custom: [{ label: 'Bad', selector: '[[' }] }, flat);
    expect(outline.custom[0]).toMatchObject({ invalid: true, count: 0 });
  });

  it('cuts a list at MAX_ENTRIES and says so', () => {
    document.body.innerHTML = '<a href="/x">x</a>'.repeat(MAX_ENTRIES + 3);
    const outline = collect({ sections: ['links'] }, flat);
    expect(outline.links).toHaveLength(MAX_ENTRIES);
    expect(outline.truncated).toEqual(['links']);
  });

  it('run() reads a malformed request as every section', () => {
    const outline = run('nonsense');
    expect(outline.headings.length).toBeGreaterThan(0);
    expect(outline.custom).toEqual([]);
  });
});
