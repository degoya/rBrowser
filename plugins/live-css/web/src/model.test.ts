import { describe, expect, it } from 'vitest';
import { type DraftState, readState, sameDraft, scopeItems, setArgs, toEdit } from './panel/model';

const STATE: DraftState = {
  available: 'trusted',
  enabled: true,
  blocks: [
    { css: 'body { margin: 0 }', breakpoints: [], scope: 'all', media: null, copy: 'body { margin: 0 }' },
    {
      css: 'body { outline: 3px solid red }',
      breakpoints: ['md'],
      scope: 'query',
      media: '(min-width: 768px) and (max-width: 1279px)',
      copy: '@media (min-width: 768px) and (max-width: 1279px) {\n  body { outline: 3px solid red }\n}',
    },
  ],
  breakpoints: [
    { id: 'mobile', label: 'Mobile', width: 375 },
    { id: 'md', label: 'MD', width: 768 },
  ],
  file: '.rbrowser/out/live-css/draft.css',
};

describe('the Live CSS panel model', () => {
  it('reads the component answer and refuses one that does not fit', () => {
    expect(readState(JSON.parse(JSON.stringify(STATE)))).toEqual(STATE);
    for (const bad of [
      null,
      [],
      { ...STATE, available: 'maybe' },
      { ...STATE, enabled: 'yes' },
      { ...STATE, blocks: [{ css: 1 }] },
      { ...STATE, blocks: [{ ...STATE.blocks[0], scope: 'some' }] },
      { ...STATE, breakpoints: [{ id: 'md', label: 'MD' }] },
      { ...STATE, file: undefined },
    ]) {
      expect(readState(bad)).toBeNull();
    }
  });

  it('edits copies of the blocks and sends them back as set arguments', () => {
    let key = 0;
    const blocks = toEdit(STATE, () => ++key);
    expect(blocks.map((block) => block.key)).toEqual([1, 2]);
    blocks[1]?.breakpoints.push('lg');
    expect(STATE.blocks[1]?.breakpoints).toEqual(['md']);
    expect(setArgs(blocks, false)).toEqual({
      blocks: [
        { css: 'body { margin: 0 }', breakpoints: [] },
        { css: 'body { outline: 3px solid red }', breakpoints: ['md', 'lg'] },
      ],
      enabled: false,
    });
  });

  it('tells whether the panel holds what the component stored', () => {
    const blocks = toEdit(STATE, () => 1);
    expect(sameDraft(blocks, true, STATE)).toBe(true);
    expect(sameDraft(blocks, false, STATE)).toBe(false);
    expect(sameDraft(blocks.slice(1), true, STATE)).toBe(false);
    const edited = toEdit(STATE, () => 1);
    if (edited[0]) edited[0].css = 'a{}';
    expect(sameDraft(edited, true, STATE)).toBe(false);
  });

  it('offers the grid breakpoints by name and keeps a chosen one that is gone, marked', () => {
    const items = scopeItems(
      STATE.breakpoints,
      ['md', 'xl'],
      (breakpoint) => `${breakpoint.label} (${breakpoint.width} px)`,
      (id) => `${id} (not in the grid)`,
    );
    expect(items).toEqual([
      { label: 'Mobile (375 px)', value: 'mobile', missing: false },
      { label: 'MD (768 px)', value: 'md', missing: false },
      { label: 'xl (not in the grid)', value: 'xl', missing: true },
    ]);
  });
});
