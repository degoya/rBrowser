import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import { STEP_SET_VIEW, type ProviderContext } from '../palette/types';
import { setViewProvider, sizeProvider } from './paletteProvider';

const t = (key: string, args?: Record<string, string | number>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function context(state: ProviderContext['state'] = sampleState()): ProviderContext {
  return { state, commands: [], settings: null, values: new Map(), t };
}

describe('sizeProvider', () => {
  it('turns a typed size into "Set view to" for the active view', () => {
    const state = sampleState();
    const view = state.activeView ?? state.views[0]?.id;
    const [entry, ...rest] = sizeProvider.forQuery?.('500x800', context()) ?? [];
    expect(rest).toEqual([]);
    expect(entry?.parent).toBe(STEP_SET_VIEW);
    expect(entry?.title).toBe('palette.entry.set_size {"width":500,"height":800}');
    expect(entry?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'view.set_size', args: { view, width: 500, height: 800 } },
    });
  });

  it('carries the pixel ratio when one is typed', () => {
    const [entry] = sizeProvider.forQuery?.('390 x 844 @3', context()) ?? [];
    expect(entry?.title).toBe('palette.entry.set_size_dpr {"width":390,"height":844,"dpr":3}');
    expect(entry?.action).toMatchObject({ message: { args: { width: 390, height: 844, dpr: 3 } } });
  });

  it('offers nothing for other text or without a view', () => {
    expect(sizeProvider.forQuery?.('tablet', context())).toEqual([]);
    expect(sizeProvider.forQuery?.('500x800', context(null))).toEqual([]);
    expect(sizeProvider.entries(context())).toEqual([]);
  });
});

describe('setViewProvider', () => {
  const device = {
    id: 'iphone-15',
    kind: 'mobile' as const,
    viewport: { width: 393, height: 852 },
    dpr: 3,
    touch: true,
  };

  it('offers every device preset in "Set view to ..." for the active view', () => {
    const entries = setViewProvider.entries({ ...context(), devices: [device, { ...device, id: 'own', label: 'My phone' }] });
    const [builtIn, own] = entries.filter((e) => e.key.startsWith('set-device:'));
    expect(builtIn).toMatchObject({
      parent: STEP_SET_VIEW,
      title: 'palette.entry.set_device {"label":"devices.iphone-15","width":393,"height":852,"dpr":3}',
      group: 'palette.kind.device',
      keywords: ['iphone-15', 'mobile'],
      action: { kind: 'send', message: { type: 'setBreakpoint', view: 1, breakpoint: 'iphone-15' } },
    });
    expect(own?.title).toContain('"label":"My phone"');
  });

  it("offers the analyser's CSS breakpoints by width, each width once", () => {
    const state = sampleState();
    state.detected = [
      { width: 1024, kind: 'min', count: 3, heuristic: false, label: 'lg' },
      { width: 768, kind: 'max', count: 1, heuristic: false },
      { width: 768, kind: 'min', count: 2, heuristic: true },
    ];
    const entries = setViewProvider.entries(context(state)).filter((e) => e.key.startsWith('set-detected:'));
    expect(entries.map((e) => e.key)).toEqual(['set-detected:768', 'set-detected:1024']);
    expect(entries[0]).toMatchObject({
      parent: STEP_SET_VIEW,
      title: 'palette.entry.set_detected {"width":768,"query":"max-width"}',
      group: 'palette.kind.detected',
      action: { kind: 'send', message: { type: 'setBreakpoint', view: 1, width: 768 } },
    });
    expect(entries[1]?.title).toBe('palette.entry.set_detected_named {"label":"lg","width":1024,"query":"min-width"}');
  });

  it('offers nothing without a view', () => {
    expect(setViewProvider.entries({ ...context(null), devices: [device] })).toEqual([]);
  });
});
