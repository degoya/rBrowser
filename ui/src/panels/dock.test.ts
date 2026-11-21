import { describe, expect, it } from 'vitest';
import { DEFAULT_STATE, KEEP_FRAMES_SETTING, NARROW_PAGE, STRIP, folded, iconsOnly, panelArgs, railSide, stateOf } from './dock';

const value = (key: string, v: unknown) => ({ key, value: v as never, origin: 'user' as const });

describe('the panels dock state', () => {
  it('reads the position, the size of each side, the fold and the side bar from the settings', () => {
    expect(
      stateOf([
        value('panels.dock', 'bottom'),
        value('panels.dockSize.left', 300),
        value('panels.dockSize.bottom', 250),
        value('panels.dockState', 'hidden'),
        value('panels.sidebar', 'icons'),
        value('ui.theme', 'dark'),
      ]),
    ).toEqual({
      position: 'bottom',
      sizes: { left: 300, right: 520, bottom: 250 },
      shown: 'hidden',
      collapsed: true,
      sidebar: 'icons',
      keepFrames: 3,
    });
    expect(stateOf([value('panels.dockState', 'open')]).collapsed).toBe(false);
  });

  it('keeps the defaults for missing or odd values', () => {
    expect(stateOf([])).toEqual(DEFAULT_STATE);
    // RB-080-93: docked right, folded to the icon rail, as the core starts.
    expect(DEFAULT_STATE.position).toBe('right');
    expect(DEFAULT_STATE.shown).toBe('rail');
    expect(DEFAULT_STATE.collapsed).toBe(true);
    expect(DEFAULT_STATE.sidebar).toBe('expanded');
    expect(
      stateOf([
        value('panels.dock', 'top'),
        value('panels.dockSize.right', 'wide'),
        value('panels.dockState', 'folded'),
        value('panels.sidebar', 'hidden'),
      ]),
    ).toEqual(DEFAULT_STATE);
  });

  it('folds only a dock, to the icon rail at its side', () => {
    const at = (position: 'left' | 'right' | 'bottom' | 'floating', collapsed: boolean) => ({
      ...DEFAULT_STATE,
      position,
      collapsed,
    });
    expect(folded(at('floating', true))).toBe(false);
    expect(folded(at('bottom', false))).toBe(false);
    expect(folded(at('bottom', true))).toBe(true);
    expect(railSide(at('bottom', true))).toBe('bottom');
    expect(railSide(at('left', true))).toBe('left');
    expect(railSide(at('right', true))).toBe('right');
    expect(railSide(at('right', false))).toBeNull();
    expect(railSide(at('floating', true))).toBeNull();
    // The core's strip (crates/rbrowser/src/panel_dock/collapse.rs): 44 DIP wide, 36 high.
    expect(STRIP).toEqual({ width: 44, height: 36 });
  });

  it('shows icons only when the setting says so or the page is narrow', () => {
    expect(iconsOnly(DEFAULT_STATE, 1024)).toBe(false);
    expect(iconsOnly({ ...DEFAULT_STATE, sidebar: 'icons' }, 1024)).toBe(true);
    expect(iconsOnly(DEFAULT_STATE, NARROW_PAGE - 1)).toBe(true);
    expect(iconsOnly(DEFAULT_STATE, NARROW_PAGE)).toBe(false);
  });

  it('names the selected panel for the commands', () => {
    expect(panelArgs('network-panel.main')).toEqual({ plugin: 'network-panel', panel: 'main' });
    expect(panelArgs(null)).toEqual({});
    expect(panelArgs('nodot')).toEqual({});
    expect(panelArgs('.x')).toEqual({});
    expect(panelArgs('x.')).toEqual({});
  });

  // RB-090-38: how many hidden panels stay loaded, a whole number up to 10.
  it('reads how many hidden frames stay loaded, the default for an odd value', () => {
    expect(DEFAULT_STATE.keepFrames).toBe(3);
    expect(stateOf([value(KEEP_FRAMES_SETTING, 0)]).keepFrames).toBe(0);
    expect(stateOf([value(KEEP_FRAMES_SETTING, 5)]).keepFrames).toBe(5);
    expect(stateOf([value(KEEP_FRAMES_SETTING, 99)]).keepFrames).toBe(10);
    expect(stateOf([value(KEEP_FRAMES_SETTING, 1.5)]).keepFrames).toBe(3);
    expect(stateOf([value(KEEP_FRAMES_SETTING, 'many')]).keepFrames).toBe(3);
  });
});
