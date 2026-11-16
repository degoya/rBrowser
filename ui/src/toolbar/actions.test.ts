import { describe, expect, it } from 'vitest';
import { TOOLBAR_COMMANDS, toMessage, type ToolbarAction } from './actions';

describe('toolbar actions', () => {
  it.each<[ToolbarAction, unknown]>([
    [{ command: 'view.back' }, { type: 'back' }],
    [{ command: 'view.forward' }, { type: 'forward' }],
    [{ command: 'reload.all' }, { type: 'reload', ignoreCache: false }],
    [{ command: 'reload.all', ignoreCache: true }, { type: 'reload', ignoreCache: true }],
    [{ command: 'view.navigate', url: 'http://localhost/' }, { type: 'navigate', url: 'http://localhost/' }],
    [{ command: 'sync.toggle', enabled: false }, { type: 'setSync', feature: 'all', enabled: false }],
    [{ command: 'layout.select_preset', id: 'focus' }, { type: 'selectLayoutPreset', id: 'focus' }],
    [{ command: 'environments.select', id: 'staging' }, { type: 'selectEnvironment', id: 'staging' }],
    [{ command: 'settings.open' }, { type: 'openSettings' }],
    [{ command: 'breakpoints.analyze' }, { type: 'runCommand', id: 'breakpoints.analyze' }],
    [{ command: 'ui.view_footer.toggle' }, { type: 'runCommand', id: 'ui.view_footer.toggle' }],
    [{ command: 'ui.theme.cycle' }, { type: 'runCommand', id: 'ui.theme.cycle' }],
    [
      { command: 'ui.theme.set', theme: 'dark' },
      { type: 'runCommand', id: 'ui.theme.set', args: { theme: 'dark' } },
    ],
    [{ command: 'panels.dock.toggle' }, { type: 'runCommand', id: 'panels.dock.toggle' }],
    [{ command: 'layout.focus_mode' }, { type: 'runCommand', id: 'layout.focus_mode' }],
    [{ command: 'layout.zoom_to_fit' }, { type: 'runCommand', id: 'layout.zoom_to_fit' }],
    [{ command: 'layout.focus_next' }, { type: 'runCommand', id: 'layout.focus_next' }],
    [
      { command: 'layout.focus_prev', source: 'arrowKey' },
      { type: 'runCommand', id: 'layout.focus_prev', args: { source: 'arrowKey' } },
    ],
    [{ command: 'layout.mode.canvas' }, { type: 'runCommand', id: 'layout.mode.canvas' }],
    [
      { command: 'layout.mode.split', args: { ratio: 0.6 } },
      { type: 'runCommand', id: 'layout.mode.split', args: { ratio: 0.6 } },
    ],
    [{ command: 'layout.save_preset' }, { type: 'runCommand', id: 'layout.save_preset' }],
    [
      { command: 'layout.save_preset', args: { id: 'board', file: 'local' } },
      { type: 'runCommand', id: 'layout.save_preset', args: { id: 'board', file: 'local' } },
    ],
    [
      { command: 'layout.center', args: { x: 10, y: 20 } },
      { type: 'runCommand', id: 'layout.center', args: { x: 10, y: 20 } },
    ],
    [{ command: 'view.add' }, { type: 'runCommand', id: 'view.add' }],
    [
      { command: 'view.add', breakpoint: 'tablet', x: 8, y: 16 },
      { type: 'runCommand', id: 'view.add', args: { breakpoint: 'tablet', x: 8, y: 16 } },
    ],
    [{ command: 'view.remove', view: 2 }, { type: 'runCommand', id: 'view.remove', args: { view: 2 } }],
    [{ command: 'view.add_size' }, { type: 'runCommand', id: 'view.add_size' }],
    [
      { command: 'view.add_size', width: 1440, height: 900, name: 'wide' },
      { type: 'runCommand', id: 'view.add_size', args: { width: 1440, height: 900, name: 'wide' } },
    ],
  ])('%j sends %j', (action, message) => {
    expect(toMessage(action)).toEqual(message);
  });

  it('names a well-formed command id for every toolbar control', () => {
    for (const command of Object.values(TOOLBAR_COMMANDS)) {
      expect(command.id).toMatch(/^[a-z0-9_]+(\.[a-z0-9_]+)+$/);
    }
  });
});
