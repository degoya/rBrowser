import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import { createPalette } from './palette';
import { createRecent } from './recent';
import { createProviderRegistry, BUILTIN_PROVIDERS } from './registry';
import { createPaletteCore, type PaletteCore } from './testCore';
import { STEP_SET_VIEW } from './types';

const t = (key: string, args?: Record<string, string | number>): string =>
  args === undefined ? key : `${key} ${JSON.stringify(args)}`;

function setup(core: PaletteCore = createPaletteCore()) {
  const state = sampleState();
  const registry = createProviderRegistry(BUILTIN_PROVIDERS);
  const palette = createPalette({
    client: core.client,
    send: async (message) => {
      await core.client.request(message);
      return true;
    },
    state: () => state,
    t,
    registry,
    recent: createRecent(null),
  });
  return { palette, core, state, registry };
}

describe('the palette', () => {
  it('opens at once and fills from the registry, the state and the settings', async () => {
    const { palette, core } = setup();
    const opening = palette.open();
    expect(palette.isOpen.value).toBe(true);
    await opening;
    const keys = palette.results.value.map((e) => e.key);
    expect(keys).toContain('command:sync.toggle');
    expect(keys).toContain('setting:sync.scroll');
    expect(keys).toContain('step:set-view');
    expect(keys).not.toContain('command:app.command_palette');
    expect(core.sent).toContainEqual({ type: 'getSettings', includeSchema: true });
  });

  it("asks for the project's flows and the device presets on every open", async () => {
    const { palette, core } = setup();
    await palette.open();
    expect(core.sent).toContainEqual({ type: 'listFlows' });
    expect(core.sent).toContainEqual({ type: 'getDevices' });
    expect(palette.results.value.map((e) => e.key)).toContain('step:run-flow');
    palette.query.value = 'login';
    expect(palette.results.value.map((e) => e.key)).toContain('flow:login');
  });

  it('runs a command, closes and gives the focus back to the view it came from', async () => {
    const { palette, core } = setup();
    await palette.open();
    palette.query.value = 'reload';
    await palette.choose();
    expect(palette.isOpen.value).toBe(false);
    const sent = core.sent.filter((m) => m.type === 'setActiveView' || m.type === 'runCommand');
    expect(sent).toEqual([
      { type: 'setActiveView', view: 1 },
      { type: 'runCommand', id: 'reload.all' },
    ]);
  });

  it('gives the focus to the first view when the one it came from is gone', async () => {
    const { palette, core, state } = setup();
    await palette.open();
    state.views = state.views.filter((v) => v.id !== 1);
    await palette.close(true);
    const focus = core.sent.filter((m) => m.type === 'setActiveView');
    expect(focus).toEqual([{ type: 'setActiveView', view: state.views[0]?.id }]);
  });

  it('forgets the view of an earlier open', async () => {
    const { palette, core, state } = setup();
    await palette.open();
    await palette.close(false);
    state.activeView = 2;
    await palette.open();
    await palette.close(true);
    expect(core.sent.filter((m) => m.type === 'setActiveView')).toEqual([{ type: 'setActiveView', view: 2 }]);
  });

  it('sends no focus without a view', async () => {
    const { palette, core, state } = setup();
    await palette.open();
    state.views = [];
    await palette.close(true);
    expect(core.sent.filter((m) => m.type === 'setActiveView')).toEqual([]);
  });

  it('hides its popup before the focus goes back, and only when asked', async () => {
    const order: string[] = [];
    const state = sampleState();
    const palette = createPalette({
      client: createPaletteCore().client,
      send: async (message) => {
        order.push(message.type);
        return true;
      },
      dismiss: async () => {
        order.push('dismiss');
      },
      state: () => state,
      t,
      registry: createProviderRegistry(BUILTIN_PROVIDERS),
      recent: createRecent(null),
    });
    await palette.open();
    await palette.close(true);
    expect(order).toEqual(['dismiss', 'setActiveView']);
    await palette.open();
    // The popup lost the focus and hid itself: nothing more to hide, no focus to take.
    await palette.close(false, false);
    expect(order).toEqual(['dismiss', 'setActiveView']);
  });

  it('sets the active view to a breakpoint through the second step', async () => {
    const { palette, core } = setup();
    await palette.open();
    palette.query.value = 'set view';
    const step = palette.results.value.find((e) => e.key === 'step:set-view');
    expect(step).toBeDefined();
    await palette.choose(step);
    expect(palette.isOpen.value).toBe(true);
    expect(palette.step.value?.id).toBe(STEP_SET_VIEW);
    expect(palette.query.value).toBe('');
    palette.query.value = 'tab';
    expect(palette.results.value[0]?.key).toBe('set-view:tablet');
    await palette.choose();
    expect(core.sent).toContainEqual({ type: 'setBreakpoint', view: 1, breakpoint: 'tablet' });
  });

  it('finds "Set view to tablet" in one step as well', async () => {
    const { palette } = setup();
    await palette.open();
    palette.query.value = 'set view to tablet';
    expect(palette.results.value[0]?.key).toBe('set-view:tablet');
  });

  it('steps back with Escape before it closes', async () => {
    const { palette } = setup();
    await palette.open();
    await palette.choose(palette.results.value.find((e) => e.key === 'step:set-view'));
    palette.escape();
    expect(palette.step.value).toBeNull();
    expect(palette.isOpen.value).toBe(true);
    palette.escape();
    expect(palette.isOpen.value).toBe(false);
  });

  it('keeps to a prefix: @ views, # settings, > commands', async () => {
    const { palette } = setup();
    await palette.open();
    palette.query.value = '@';
    expect(palette.results.value.every((e) => e.scope === 'view')).toBe(true);
    expect(palette.results.value.length).toBeGreaterThan(0);
    palette.query.value = '#scroll';
    expect(palette.results.value.map((e) => e.key)).toEqual(['setting:sync.scroll']);
    palette.query.value = '>';
    expect(palette.results.value.every((e) => e.scope === 'command')).toBe(true);
  });

  it('takes the prefixes as text when palette.prefixes is off', async () => {
    const core = createPaletteCore();
    core.values.push({ key: 'palette.prefixes', value: false, origin: 'user' });
    const { palette } = setup(core);
    await palette.open();
    expect(palette.options.value.prefixes).toBe(false);
    palette.query.value = '#';
    expect(palette.results.value).toEqual([]);
  });

  it('lists the last used entries first next time', async () => {
    const { palette } = setup();
    await palette.open();
    palette.query.value = 'zoom fit';
    await palette.choose();
    await palette.open();
    expect(palette.results.value[0]?.key).toBe('command:view.zoom_fit_all');
  });

  it('shows a plugin command as soon as the registry has it, and drops it again', async () => {
    const { palette, core } = setup();
    core.commands.push({
      id: 'plugin.acme.lint',
      titleKey: 'commands.plugin.acme.lint',
      group: 'plugins',
      bindings: [],
      defaultBindings: [],
      changed: false,
      mcp: { omitted: 'test' },
    });
    await palette.open();
    expect(palette.results.value.map((e) => e.key)).toContain('command:plugin.acme.lint');
    palette.close(false);
    core.commands.pop();
    await palette.open();
    expect(palette.results.value.map((e) => e.key)).not.toContain('command:plugin.acme.lint');
  });

  it('opens a command in the shortcut editor with Ctrl+Enter', async () => {
    const { palette, core } = setup();
    await palette.open();
    palette.query.value = 'sync toggle';
    await palette.editShortcut();
    expect(core.sent).toContainEqual({ type: 'openSettings', key: 'shortcuts.bindings' });
    expect(palette.isOpen.value).toBe(false);
  });

  it('moves the selection within the results and wraps around', async () => {
    const { palette } = setup();
    await palette.open();
    palette.query.value = 'o';
    const count = palette.results.value.length;
    expect(count).toBeGreaterThan(2);
    palette.move(1);
    expect(palette.active.value).toBe(1);
    palette.move(-2);
    expect(palette.active.value).toBe(count - 1);
    palette.query.value = 'ov';
    expect(palette.active.value).toBe(0);
  });

  it('asks a query provider on every keystroke', async () => {
    const { palette, registry } = setup();
    registry.register({
      id: 'sizes',
      entries: () => [],
      forQuery: (query) =>
        /^\d+x\d+$/.test(query)
          ? [{ key: 'size', scope: 'other', title: `Set view to ${query}`, parent: STEP_SET_VIEW, action: { kind: 'send', message: { type: 'runCommand', id: 'view.set_size', args: { width: 500, height: 800 } } } }]
          : [],
    });
    await palette.open();
    palette.query.value = '500x800';
    expect(palette.results.value.map((e) => e.key)).toEqual(['size']);
  });
  // RB-080-46: `view.add_size` opens the palette in its step - the new-size picker.
  it('opens in a step of its own, which Escape closes and Backspace leaves', async () => {
    const { palette } = setup();
    await palette.open({ id: STEP_SET_VIEW, title: 'Set view' });
    expect(palette.step.value).toEqual({ id: STEP_SET_VIEW, title: 'Set view' });
    expect(palette.results.value.length).toBeGreaterThan(0);
    expect(palette.results.value.every((entry) => entry.parent === STEP_SET_VIEW)).toBe(true);
    palette.escape();
    expect(palette.isOpen.value).toBe(false);
    await palette.open({ id: STEP_SET_VIEW, title: 'Set view' });
    palette.back();
    expect(palette.isOpen.value).toBe(true);
    expect(palette.step.value).toBeNull();
    // A step chosen from the top level is left by Escape, as before.
    await palette.open();
    await palette.choose(palette.results.value.find((entry) => entry.key === 'step:set-view'));
    palette.escape();
    expect(palette.isOpen.value).toBe(true);
    expect(palette.step.value).toBeNull();
  });
});
