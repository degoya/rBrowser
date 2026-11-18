// The menu coverage (RB-090-63), like the MCP coverage: every command of the registry has a
// visible way - a row of the main menu `≡` or a split button's menu (`mainMenu.ts`,
// `buttonMenus.ts`), or another visible control named here (`OTHER_WAYS`) - or says why it has
// none (`MENU_OMITTED`). A plugin's commands land in the submenu of their leader group, so every
// one has a row. `coverage.test.ts` holds the registry (`schema/commands.v1.json`) and the bundled
// plugins' manifests to it: a new command without a way fails it.

/** Commands reached by a visible control outside the menus: the control. */
export const OTHER_WAYS: Readonly<Record<string, string>> = {
  'view.navigate': 'toolbar: the address bar',
  'view.focus_address_bar': 'toolbar: the address bar',
  'view.force_state.set': 'page menu: Force state (it needs the element under the click)',
  'portal.add_asset_origin': "status bar: the portal's offer to add an origin it saw (it needs that origin)",
  'update.check': 'Settings > General > Updates: Check now',
  'update.install': "Settings > General > Updates and the update notice's Install and restart",
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `view.set_breakpoint.${index + 1}`,
      "a view's label strip: the size quick pick lists the breakpoints",
    ]),
  ),
};

/** Commands with no menu row of their own, and why. */
export const MENU_OMITTED: Readonly<Record<string, string>> = {
  'ui.toolbar.overflow.open': 'opens the main menu itself',
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `layout.preset.${index + 1}`,
      'numbered family: Layout > Presets lists every preset (layout.select_preset)',
    ]),
  ),
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `environments.switch.${index + 1}`,
      'numbered family: Environments lists every environment (environments.select)',
    ]),
  ),
};
