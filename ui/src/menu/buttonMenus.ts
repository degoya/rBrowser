// The menus of the toolbar's split buttons (RB-090-63, `SplitButton.vue`): DevTools, Screenshot,
// Video and Checks. A click on the button runs its main command (`BUTTONS`); the arrow beside it
// opens the rest as the core's native menu. The main menu `≡` (`mainMenu.ts`) has the same trees
// as submenus, so a row has one definition.

import type { DevtoolsMode } from '../ipc/protocol';
import { settingOn, type MenuContext, type MenuNode } from './menuModel';

const sep: MenuNode = { kind: 'separator' };
const cmd = (id: string, extra: Partial<Extract<MenuNode, { kind: 'command' }>> = {}): MenuNode => ({
  kind: 'command',
  id,
  ...extra,
});

/** DevTools move to the view that became active. */
const FOLLOW_SETTING = 'devtools.followActiveView';

/** Where DevTools open (`devtools.mode`), as choices that write the setting. */
const DOCK_MODES: readonly [DevtoolsMode, string][] = [
  ['docked-bottom', 'shell.devtools.mode.docked_bottom'],
  ['docked-right', 'shell.devtools.mode.docked_right'],
  ['window', 'shell.devtools.mode.window'],
];

/** The dock choices: the mode in effect checked; a choice writes `devtools.mode` for the user. */
function dockRows(ctx: MenuContext): MenuNode[] {
  const current = ctx.settings['devtools.mode'] ?? 'docked-bottom';
  return DOCK_MODES.map(([mode, key]) => ({
    kind: 'row',
    label: ctx.t(key),
    message: { type: 'setSetting', key: 'devtools.mode', value: mode, scope: 'user' },
    checked: current === mode,
    radio: true,
  }));
}

/** DevTools follow the active view (`devtools.followActiveView`): a check row that flips it. */
function followRow(ctx: MenuContext): MenuNode[] {
  const on = settingOn(ctx, FOLLOW_SETTING);
  return [
    {
      kind: 'row',
      label: ctx.t('shell.menu.devtools_follow'),
      message: { type: 'setSetting', key: FOLLOW_SETTING, value: !on, scope: 'user' },
      checked: on,
    },
  ];
}

export const DEVTOOLS_MENU: readonly MenuNode[] = [
  cmd('devtools.toggle'),
  cmd('devtools.inspect_at'),
  cmd('devtools.open'),
  sep,
  { kind: 'list', covers: [], build: dockRows },
  cmd('devtools.dock_cycle'),
  { kind: 'list', covers: [], build: followRow },
  sep,
  cmd('view.force_state.reset'),
  cmd('plugin.framework_devtools.vue'),
  cmd('plugin.framework_devtools.react'),
  sep,
  cmd('debug.vitals', { checked: (ctx) => settingOn(ctx, 'vitals.enabled') }),
  cmd('debug.resources', { checked: (ctx) => settingOn(ctx, 'resources.enabled') }),
  // The timing counters' summary (RB-090-32; MCP app_perf has them all).
  cmd('debug.perf_counters'),
  { kind: 'plugins', group: 'devtools' },
];

export const CAPTURE_MENU: readonly MenuNode[] = [
  cmd('capture.views'),
  cmd('capture.element'),
  cmd('capture.all_viewport'),
  cmd('capture.full_page'),
  cmd('capture.overview'),
  sep,
  cmd('capture.delay_cycle'),
  cmd('capture.toggle_hide_banners', { checked: (ctx) => settingOn(ctx, 'capture.hideBanners') }),
  cmd('capture.toggle_freeze', { checked: (ctx) => settingOn(ctx, 'capture.freezeAnimations') }),
  cmd('capture.preview_clean'),
  cmd('plugin.screenshot_editor.open_last'),
];

export const VIDEO_MENU: readonly MenuNode[] = [
  cmd('capture.video.toggle'),
  cmd('capture.video.toggle_all'),
  // RB-090-68: checked while the running recording is paused; no choice without one.
  cmd('capture.video.pause', {
    checked: (ctx) => ctx.state?.video?.pausedAtMs !== undefined,
    enabled: (ctx) => ctx.state?.video !== undefined,
  }),
  cmd('capture.video.panel'),
  sep,
  cmd('recorder.start'),
];

export const CHECKS_MENU: readonly MenuNode[] = [
  cmd('checks.run_all'),
  cmd('checks.cancel'),
  cmd('checks.export'),
  sep,
  { kind: 'plugins', group: 'checks' },
  sep,
  cmd('breakpoints.analyze'),
  cmd('panels.findings'),
];

/** The split buttons: the main command a click runs, the menu of the arrow. */
export const BUTTONS = {
  devtools: { command: 'devtools.toggle', menu: DEVTOOLS_MENU },
  capture: { command: 'capture.views', menu: CAPTURE_MENU },
  video: { command: 'capture.video.toggle', menu: VIDEO_MENU },
  checks: { command: 'checks.run_all', menu: CHECKS_MENU },
} as const;
