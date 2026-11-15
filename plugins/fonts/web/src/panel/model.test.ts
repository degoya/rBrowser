import { describe, expect, it } from 'vitest';
import { type Answer, type FaceEntry, faceStates, findingsFor, message, renderedKnown, renderedState, roleCells, roleViews, rows, text } from './model';

const strings = {
  'plugin.fonts.panel.failed': 'Failed: {code}',
  'plugin.fonts.panel.title': 'Fonts',
  'plugin.fonts.findings.rendered-fallback': 'Declared {declared}, rendered {rendered}.',
};

const answer: Answer = {
  views: [
    { id: 1, label: 'Desktop', width: 1280, engine: 'chromium' },
    { id: 2, label: 'Mobile', width: 375, engine: 'chromium' },
  ],
  families: [
    {
      family: 'Unused Display',
      provider: 'own',
      host: 'localhost',
      webFont: true,
      formats: ['woff2'],
      variable: false,
      faces: [],
      fileKb: null,
      usage: [],
      rendered: [],
    },
    {
      family: 'Inter',
      provider: 'own',
      host: 'localhost',
      webFont: true,
      formats: ['woff2'],
      variable: false,
      faces: [],
      fileKb: 34.4,
      usage: [
        { view: 1, elements: 8, weights: [400, 600], styles: ['normal'], sizes: [16, 32], },
        { view: 2, elements: 7, weights: [400], styles: ['normal'], sizes: [16] },
      ],
      rendered: [
        { view: 1, state: 'ok', as: [] },
        { view: 2, state: 'fallback', as: ['Arial'] },
      ],
    },
  ],
  roles: [{ role: 'h1', views: [{ view: 2, width: 375, min: 32, max: 32 }], growth: null, scaled: null, jump: false }],
  findings: [
    { rule: 'fonts.rendered-fallback', severity: 'error', message: 'plugin.fonts.findings.rendered-fallback', params: { declared: 'Inter', rendered: 'Arial' }, view: 2, path: 'p' },
    { rule: 'fonts.face-unused', severity: 'info', message: 'x', params: { family: 'unused display' }, view: 1, path: '@font-face' },
  ],
};

describe('the panel model', () => {
  it('fills texts and placeholders, the key when a text is missing', () => {
    expect(text(strings, 'panel.title')).toBe('Fonts');
    expect(text(strings, 'panel.nope')).toBe('panel.nope');
    expect(message(strings, answer.findings?.[0] as never)).toBe('Declared Inter, rendered Arial.');
  });

  it('lists the families in use first, with their use per view and the worst rendered state', () => {
    const list = rows(answer);
    expect(list.map((row) => row.family)).toEqual(['Inter', 'Unused Display']);
    expect(list[0]).toMatchObject({ elements: 15, rendered: 'fallback', findings: 1, perView: 'Desktop 8 · Mobile 7' });
    expect(list[1]).toMatchObject({ elements: 0, rendered: undefined, findings: 1 });
  });

  it('finds the findings of a family by name, case-blind', () => {
    expect(findingsFor(answer, 'UNUSED display').map((f) => f.rule)).toEqual(['fonts.face-unused']);
    expect(renderedState(answer.families[0] as never)).toBeUndefined();
  });

  it('gives a breakpoint cell per view, a dash where the role is missing', () => {
    expect(roleCells(answer, answer.roles[0] as never)).toEqual(['–', '32']);
  });

  // RB-080-62: the owner's Fonts panel showed "Rendered: unknown" in every row, a breakpoint column
  // of dashes for a Firefox view and a status per view where every view said "loaded".
  it('shows the Rendered column only when a view knows how the family was drawn', () => {
    const inter = answer.families[1] as never;
    expect(renderedKnown(inter)).toBe(true);
    const unknown = { ...answer.families[1], rendered: [{ view: 1, state: 'unknown', as: [] }, { view: 2, state: 'unknown', as: [] }] };
    expect(renderedKnown(unknown as never)).toBe(false);
    expect(renderedKnown(answer.families[0] as never)).toBe(false);
  });

  it('leaves out the breakpoint columns of views without a size', () => {
    expect(roleViews(answer).map((view) => view.label)).toEqual(['Mobile']);
    expect(roleCells(answer, answer.roles[0] as never, roleViews(answer))).toEqual(['32']);
  });

  it("groups a face's states over the views, one entry when they all agree", () => {
    const face = (states: string[]): FaceEntry => ({
      weight: '400', style: 'normal', display: 'swap', unicodeRange: '', src: null, format: 'woff2', kb: 1, preloaded: false,
      status: states.map((state, index) => ({ view: index + 1, state })),
    });
    expect(faceStates(answer, face(['loaded', 'loaded']))).toEqual([{ state: 'loaded', views: [] }]);
    // Review of RB-080-62 (4): one state from only some of the views names them.
    expect(faceStates(answer, face(['loaded']))).toEqual([{ state: 'loaded', views: ['Desktop'] }]);
    const firefox: Answer = { ...answer, views: [...answer.views, { id: 3, label: 'Firefox', width: 1280, engine: 'firefox', error: 'unsupported' }] };
    expect(faceStates(firefox, face(['loaded', 'loaded']))).toEqual([{ state: 'loaded', views: [] }]);
    expect(faceStates(answer, face(['loaded', 'error']))).toEqual([
      { state: 'loaded', views: ['Desktop'] },
      { state: 'error', views: ['Mobile'] },
    ]);
    expect(faceStates(answer, face([]))).toEqual([]);
  });
});
