// The always-visible main menu `≡` (RB-090-63): every function of rBrowser by name, so nothing is
// reachable only by a shortcut. On top the command palette, what left the toolbar's row and the
// bookmarks (the owner's word: directly, not in a submenu); then a submenu per area - View,
// Layout, Sync, Emulation, Environments, Capture, Checks, Overlays, Tools, Flows, Panels,
// Plugins & extensions, App - with check marks, radio choices and the shortcut beside each row.
// The rows are commands of the one registry (`menuModel.ts`); the split buttons' menus
// (`buttonMenus.ts`) are submenus here too, so their rows have one definition.

import { bookmarksOf, groupBookmarks, openMessage, ADD_COMMAND, OPEN_COMMAND } from '../bookmarks/bookmarks';
import { THEME_CHOICES, THEME_CYCLE_COMMAND, THEME_SET_COMMAND, themeChoiceKey } from '../toolbar/themeToggle';
import { CAPTURE_MENU, CHECKS_MENU, DEVTOOLS_MENU, VIDEO_MENU } from './buttonMenus';
import { commandMessage, settingOn, type MenuContext, type MenuNode } from './menuModel';

/** The most bookmarks the main menu lists itself; "All bookmarks..." opens the rest. */
export const MENU_BOOKMARKS = 25;

const sep: MenuNode = { kind: 'separator' };
const cmd = (id: string, extra: Partial<Extract<MenuNode, { kind: 'command' }>> = {}): MenuNode => ({
  kind: 'command',
  id,
  ...extra,
});
const sub = (labelKey: string, children: readonly MenuNode[]): MenuNode => ({ kind: 'submenu', labelKey, children });
/** A command whose check mark is a boolean setting. */
const setting = (id: string, key: string, fallback = false): MenuNode =>
  cmd(id, { checked: (ctx) => settingOn(ctx, key, fallback) });

const activeView = (ctx: MenuContext) => ctx.state?.activeView ?? null;
const layoutMode = (mode: string) => (ctx: MenuContext) => ctx.state?.layout.mode === mode;

/** The project's bookmarks on the main menu's top level, grouped, then add and search. */
function bookmarkRows(ctx: MenuContext): MenuNode[] {
  const out: MenuNode[] = [];
  let listed = 0;
  for (const group of groupBookmarks(bookmarksOf(ctx.state))) {
    if (listed >= MENU_BOOKMARKS) break;
    if (group.name !== null) out.push({ kind: 'heading', label: group.name });
    for (const bookmark of group.items) {
      if (listed >= MENU_BOOKMARKS) break;
      listed += 1;
      out.push({
        kind: 'row',
        label: bookmark.label,
        message: openMessage(bookmark.id),
        disabled: bookmark.url === undefined,
      });
    }
  }
  return out;
}

/** The layout presets as choices, the one in effect checked. */
function presetRows(ctx: MenuContext): MenuNode[] {
  const layout = ctx.state?.layout;
  return (layout?.presets ?? []).map((id) => ({
    kind: 'row',
    label: id,
    message: commandMessage('layout.select_preset', { id }),
    checked: layout?.preset === id,
    radio: true,
  }));
}

/** The environments as choices, the one in effect checked. */
function environmentRows(ctx: MenuContext): MenuNode[] {
  const state = ctx.state;
  return (state?.environments ?? []).map((env) => ({
    kind: 'row',
    label: env.id,
    message: commandMessage('environments.select', { id: env.id }),
    checked: state?.environment === env.id,
    radio: true,
  }));
}

/** `breakpoints.detected.1` to `.9`: the analyser's finds as sizes for the active view. */
const DETECTED = Array.from({ length: 9 }, (_, index) => `breakpoints.detected.${index + 1}`);

/** The breakpoints the analyser found, each a size for the active view. */
function detectedRows(ctx: MenuContext): MenuNode[] {
  return (ctx.state?.detected ?? []).slice(0, DETECTED.length).map((_, index) => cmd(DETECTED[index] ?? ''));
}

/** rBrowser's theme as choices (`ui.theme.set`), the one in effect checked. */
function themeRows(ctx: MenuContext): MenuNode[] {
  const current = ctx.settings['ui.theme'] ?? 'system';
  return THEME_CHOICES.map((theme) => ({
    kind: 'row',
    label: ctx.t(themeChoiceKey(theme, ctx.systemTheme ?? 'light')),
    message: commandMessage(THEME_SET_COMMAND, { theme }),
    checked: current === theme,
    radio: true,
  }));
}

export const VIEW_MENU: readonly MenuNode[] = [
  cmd('view.add'),
  cmd('view.add_size'),
  cmd('view.add_print'),
  cmd('view.remove'),
  sep,
  cmd('view.set_size'),
  cmd('view.reset_size'),
  cmd('view.next_breakpoint'),
  cmd('view.prev_breakpoint'),
  cmd('view.nudge_width.plus'),
  cmd('view.nudge_width.minus'),
  cmd('view.set_engine'),
  sep,
  sub('shell.menu.page_zoom', [cmd('view.page_zoom.in'), cmd('view.page_zoom.out'), cmd('view.page_zoom.reset')]),
  // The view's scale in its cell, apart from the page's zoom (which changes the page's CSS width).
  sub('shell.menu.view_scale', [cmd('view.zoom_in'), cmd('view.zoom_out'), cmd('view.zoom_fit_all')]),
  sub('shell.menu.external', [cmd('view.pop_out'), cmd('view.pop_in'), cmd('view.external.raise'), cmd('view.external.arrange')]),
  cmd('view.focus_next'),
  cmd('view.focus_prev'),
  sep,
  cmd('view.back'),
  cmd('view.forward'),
  cmd('reload.all'),
  sep,
  cmd('ui.view_footer.toggle', {
    labelKey: 'shell.toolbar.footer',
    checked: (ctx) => settingOn(ctx, 'ui.viewFooter.visible', true),
  }),
  // The vitals bar below the breakpoint bar (RB-090-71).
  cmd('ui.vitals_bar.toggle', {
    labelKey: 'shell.toolbar.vitalsBar',
    checked: (ctx) => settingOn(ctx, 'ui.viewFooter.vitals', true),
  }),
  // The Lighthouse bar below the vitals bar (RB-090-73).
  cmd('ui.lighthouse_bar.toggle', {
    labelKey: 'shell.toolbar.lighthouseBar',
    checked: (ctx) => settingOn(ctx, 'ui.viewFooter.lighthouse', true),
  }),
  setting('status.toggle_bar', 'ui.statusBar.visible', true),
  setting('performance.toggle_throttle', 'performance.throttleHidden', true),
  { kind: 'plugins', group: 'view' },
];

export const LAYOUT_MENU: readonly MenuNode[] = [
  cmd('layout.mode.grid', { radio: true, checked: layoutMode('grid') }),
  cmd('layout.mode.horizontal', { radio: true, checked: layoutMode('horizontal') }),
  cmd('layout.mode.split', { radio: true, checked: layoutMode('split') }),
  cmd('layout.mode.canvas', { radio: true, checked: layoutMode('canvas') }),
  // The command toggles; `enabled` makes the row a choice like the modes beside it.
  cmd('layout.focus_mode', { radio: true, checked: layoutMode('focus'), args: { enabled: true } }),
  cmd('layout.focus_next'),
  cmd('layout.focus_prev'),
  cmd('layout.zoom_to_fit'),
  sep,
  sub('shell.menu.presets', [
    { kind: 'list', covers: ['layout.select_preset'], build: presetRows },
    sep,
    cmd('layout.save_preset'),
  ]),
  sub('shell.menu.split', [cmd('layout.split.swap'), cmd('layout.split.grow'), cmd('layout.split.shrink')]),
  sub('shell.menu.canvas', [
    cmd('canvas.zoom.in'),
    cmd('canvas.zoom.out'),
    cmd('canvas.zoom.reset'),
    cmd('layout.center'),
    cmd('layout.canvas.place'),
  ]),
  setting('layout.minimap.toggle', 'layoutModes.minimap'),
  sep,
  sub('shell.menu.breakpoints', [
    cmd('breakpoints.analyze'),
    cmd('breakpoints.append_detected'),
    { kind: 'list', covers: DETECTED, build: detectedRows },
    { kind: 'plugins', group: 'breakpoints' },
  ]),
  { kind: 'plugins', group: 'layout' },
];

export const SYNC_MENU: readonly MenuNode[] = [
  cmd('sync.toggle', { checked: (ctx) => ctx.state?.sync.enabled ?? false }),
  cmd('sync.toggle_scroll', { checked: (ctx) => ctx.state?.sync.features.scroll ?? false }),
  setting('sync.toggle_focus', 'sync.focus'),
  cmd('sync.toggle_view', {
    checked: (ctx) => !(ctx.state?.sync.excluded ?? []).includes(activeView(ctx) ?? -1),
    enabled: (ctx) => activeView(ctx) !== null,
  }),
  cmd('sync.toggle_view_navigation', {
    checked: (ctx) => !(ctx.state?.sync.navigationDecoupled ?? []).includes(activeView(ctx) ?? -1),
    enabled: (ctx) => activeView(ctx) !== null,
  }),
  cmd('sync.resync'),
  cmd('sync.skip_next', { checked: (ctx) => ctx.state?.sync.skipNextClick ?? false }),
  sep,
  setting('page_api.toggle', 'pageApi.enabled'),
  setting('page_api.toggle_ua_suffix', 'pageApi.uaSuffix'),
  setting('bus.toggle', 'bus.enabled'),
  sep,
  sub('shell.menu.portal', [cmd('portal.toggle'), cmd('portal.show_qr'), cmd('portal.devices'), cmd('portal.disconnect')]),
  { kind: 'plugins', group: 'sync' },
];

export const EMULATION_MENU: readonly MenuNode[] = [
  sub('shell.menu.color_scheme', [
    cmd('emulation.set', { labelKey: 'shell.menu.scheme_light', args: { fields: { colorScheme: 'light' } } }),
    cmd('emulation.set', { labelKey: 'shell.menu.scheme_dark', args: { fields: { colorScheme: 'dark' } } }),
    cmd('emulation.toggle_color_scheme'),
  ]),
  cmd('emulation.toggle_reduced_motion'),
  cmd('view.emulate.offline'),
  cmd('view.emulate.geolocation'),
  cmd('view.emulate.timezone'),
  cmd('view.emulate.cpu'),
  sep,
  cmd('emulation.reset_view'),
  cmd('emulation.reset_view', { labelKey: 'shell.menu.reset_all_views', args: { target: 'all' } }),
];

export const ENVIRONMENTS_MENU: readonly MenuNode[] = [
  { kind: 'list', covers: ['environments.select'], build: environmentRows },
  sep,
  sub('shell.menu.compare', [cmd('env.compare.live'), cmd('env.compare.run'), cmd('env.compare.report'), cmd('env.compare.cancel')]),
  sub('shell.menu.sessions', [cmd('session.assign'), cmd('session.clear'), cmd('cookies.copy')]),
  { kind: 'plugins', group: 'environments' },
];

export const CAPTURE_SUBMENU: readonly MenuNode[] = [
  sub('shell.menu.screenshot', CAPTURE_MENU),
  sub('shell.menu.video', VIDEO_MENU),
  sub('shell.menu.visual', [cmd('visual.compare'), cmd('visual.baseline_update'), cmd('visual.lfs_setup')]),
  sub('shell.menu.sitemap', [cmd('sitemap.capture'), cmd('sitemap.cancel'), cmd('panels.sitemap')]),
  { kind: 'plugins', group: 'capture' },
];

export const TOOLS_MENU: readonly MenuNode[] = [
  sub('shell.menu.devtools', DEVTOOLS_MENU),
  sub('shell.menu.reload', [cmd('reload.css_only'), cmd('reload.toggle_pause'), { kind: 'plugins', group: 'reload' }]),
  sub('shell.menu.engines', [
    cmd('engine.restart'),
    cmd('sidecar.restart'),
    cmd('wsl.restart_helper'),
    cmd('engine.firefox.toggle'),
    cmd('engine.webkit.toggle', { checked: (ctx) => settingOn(ctx, 'engines.webkit.enabled') }),
    cmd('engines.update'),
  ]),
];

export const FLOWS_MENU: readonly MenuNode[] = [
  cmd('flows.run'),
  cmd('flows.run_all'),
  cmd('flows.rerun_last'),
  cmd('flows.cancel'),
  cmd('flows.open_results'),
  cmd('panels.flows'),
  sep,
  sub('shell.menu.recorder', [
    cmd('recorder.start'),
    cmd('recorder.pause'),
    cmd('recorder.stop'),
    cmd('recorder.check'),
    cmd('recorder.play'),
    cmd('recorder.export_flow'),
    cmd('recorder.export_spec'),
    cmd('panels.recorder'),
  ]),
  cmd('cdp.copy_snippet'),
  { kind: 'plugins', group: 'flows' },
];

export const PANELS_MENU: readonly MenuNode[] = [
  { kind: 'plugins', group: 'panels' },
  cmd('panels.findings'),
  cmd('status.history'),
  cmd('plugins.panels'),
  sep,
  cmd('panels.dock.toggle'),
  cmd('panels.dock.left'),
  cmd('panels.dock.right'),
  cmd('panels.dock.bottom'),
  cmd('panels.undock'),
  cmd('panels.close'),
];

export const PLUGINS_MENU: readonly MenuNode[] = [
  cmd('plugins.list'),
  cmd('plugins.catalog'),
  cmd('plugins.install'),
  cmd('plugins.enable'),
  cmd('plugins.disable'),
  cmd('plugins.remove'),
  cmd('plugins.resume'),
  cmd('plugins.check_updates'),
  cmd('plugins.trust_key'),
  sep,
  sub('shell.menu.extensions', [
    cmd('extensions.manage'),
    cmd('extensions.install_web_store'),
    cmd('extensions.load_unpacked'),
    cmd('extensions.add_recommended'),
    cmd('extensions.check_updates'),
    cmd('extensions.remove'),
  ]),
  sub('shell.menu.ffmpeg', [cmd('tools.ffmpeg.install'), cmd('tools.ffmpeg.remove')]),
  { kind: 'plugins', group: 'plugins' },
];

export const APP_MENU: readonly MenuNode[] = [
  cmd('settings.open'),
  cmd('settings.open_plugins'),
  cmd('shortcuts.edit'),
  sub('shell.menu.theme', [{ kind: 'list', covers: [THEME_SET_COMMAND], build: themeRows }, sep, cmd(THEME_CYCLE_COMMAND)]),
  sub('shell.menu.trust', [cmd('trust.show'), cmd('trust.grant'), cmd('trust.revoke')]),
  cmd('project.open_recent'),
  cmd('app.reload_config'),
  sub('shell.menu.mcp', [setting('mcp.toggle', 'mcp.enabled'), cmd('mcp.copy_config')]),
  sub('shell.menu.cdp', [cmd('cdp.copy_endpoint'), cmd('cdp.rotate_token')]),
  sep,
  cmd('diagnostics.create'),
  cmd('app.open_link', { labelKey: 'shell.menu.handbook' }),
  cmd('app.about'),
  { kind: 'plugins', group: 'settings' },
];

/** The main menu below the overflowed toolbar groups (`MainMenu.vue` puts those on top). */
export const MAIN_MENU: readonly MenuNode[] = [
  cmd('app.command_palette'),
  sep,
  { kind: 'list', covers: [], build: bookmarkRows },
  cmd(ADD_COMMAND),
  cmd(OPEN_COMMAND),
  cmd('bookmarks.make_relative'),
  sep,
  sub('shell.menu.view', VIEW_MENU),
  sub('shell.menu.layout', LAYOUT_MENU),
  sub('shell.menu.sync', SYNC_MENU),
  sub('shell.menu.emulation', EMULATION_MENU),
  sub('shell.menu.environments', ENVIRONMENTS_MENU),
  sub('shell.menu.capture', CAPTURE_SUBMENU),
  sub('shell.menu.checks', CHECKS_MENU),
  sub('shell.menu.overlays', [{ kind: 'plugins', group: 'overlays' }]),
  sub('shell.menu.tools', TOOLS_MENU),
  sub('shell.menu.flows', FLOWS_MENU),
  sub('shell.menu.panels', PANELS_MENU),
  sub('shell.menu.plugins', PLUGINS_MENU),
  sub('shell.menu.app', APP_MENU),
];
