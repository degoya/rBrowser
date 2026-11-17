import { describe, expect, it } from 'vitest';
import type { DevicePreset, JsonValue } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { STEP_NEW_VIEW, type ProviderContext } from '../palette/types';
import { newViewProvider } from './newViewProvider';

const t = (key: string, args?: Record<string, string | number>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

const DEVICES: DevicePreset[] = [
  { id: 'desk', kind: 'desktop', viewport: { width: 1920, height: 1080 }, dpr: 1, touch: false },
  { id: 'phone', label: 'My phone', kind: 'mobile', viewport: { width: 390, height: 844 }, dpr: 3, touch: true },
];

function context(settings: Record<string, JsonValue> = {}): ProviderContext {
  const state = sampleState();
  const hidden = state.breakpoints[0]?.id ?? '';
  state.layout.hidden = [hidden];
  state.detected = [{ width: 640, kind: 'min', label: 'sm', count: 3, heuristic: false }];
  return { state, commands: [], settings: null, values: new Map(Object.entries(settings)), devices: DEVICES, t };
}

describe('the new-view picker of the palette', () => {
  it('lists the sizes, the devices and Custom, all in its step', () => {
    const ctx = context();
    const entries = newViewProvider.entries(ctx);
    expect(entries.every((entry) => entry.parent === STEP_NEW_VIEW)).toBe(true);
    const hidden = ctx.state?.layout.hidden?.[0];
    // RB-080-68: "Custom size..." first and pinned there; it stood last, past the popup's ten rows.
    expect(entries.map((entry) => entry.key)).toEqual([
      'new-view:custom',
      `new-view:breakpoint:${hidden}`,
      'new-view:detected:640',
      'new-view:device:phone',
      'new-view:device:desk',
    ]);
    expect(entries[0]?.pinned).toBe(true);
    expect(entries.slice(1).some((entry) => entry.pinned === true)).toBe(false);
    // A hidden breakpoint opens again; a detected width becomes a size of the active view's height.
    expect(entries[1]?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'view.add', args: { breakpoint: hidden } },
    });
    const height = ctx.state?.views.find((view) => view.id === ctx.state?.activeView)?.height;
    expect(entries[2]?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'view.add_size', args: { width: 640, height } },
    });
    // A device goes by its id: the core takes its size and pixel ratio from the preset.
    expect(entries[3]?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'view.add_size', args: { device: 'phone' } },
    });
    expect(entries[3]?.title).toContain('My phone');
    expect(entries[3]?.title).toContain('"dpr":3');
    expect(entries[0]?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'view.add_size', args: { custom: true } },
    });
  });

  it('leaves the devices out with ui.newViewSize.devices off', () => {
    const keys = newViewProvider.entries(context({ 'ui.newViewSize.devices': false })).map((entry) => entry.key);
    expect(keys.some((key) => key.startsWith('new-view:device:'))).toBe(false);
    expect(keys[0]).toBe('new-view:custom');
  });

  it('offers a typed size as a view of its own', () => {
    const typed = newViewProvider.forQuery?.('1000x1000@2', context()) ?? [];
    expect(typed).toHaveLength(1);
    expect(typed[0]?.parent).toBe(STEP_NEW_VIEW);
    expect(typed[0]?.action).toEqual({
      kind: 'send',
      message: { type: 'runCommand', id: 'view.add_size', args: { width: 1000, height: 1000, dpr: 2 } },
    });
    expect(newViewProvider.forQuery?.('iphone', context())).toEqual([]);
  });
});
