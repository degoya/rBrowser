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
  'portal.present.new_link': "the Portal panel's presentation section: New link",
  'portal.present.stop': "the Portal panel's presentation section and the activity bar: End presentation",
  'portal.add_asset_origin': "status bar: the portal's offer to add an origin it saw (it needs that origin)",
  'update.check': 'Settings > General > Updates: Check now',
  'update.install': "Settings > General > Updates and the update notice's Install and restart",
  'flows.delete': 'the Flows panel: Delete beside each flow (it needs the flow)',
  'recorder.delete': 'the Recorder panel: Delete beside each recording (it needs the recording)',
  'view.edit_address': "a decoupled view's address in its label strip, and the page menu's Sync (it needs the view)",
  'recorder.step_add': "the Recorder panel: + between two steps and Add step (it needs the place)",
  'recorder.step_try': "the Recorder panel: Try beside each step (it needs the step)",
  'recorder.step_toggle': "the Recorder panel: a step's menu, Skip / Include (it needs the step)",
  'recorder.play_from': "the Recorder panel: a step's menu, Play from here (it needs the step)",
  'recorder.continue_from': "the Recorder panel: a step's menu, Continue recording from here (it needs the step)",
  'recorder.pick_element': "the Recorder panel: Pick in view in a step's editor (it needs the step)",
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [
      `view.set_breakpoint.${index + 1}`,
      "a view's label strip: the size quick pick lists the breakpoints",
    ]),
  ),
};

/**
 * Commands with no menu row of their own, and why: only the main menu's own opener and the
 * numbered families another row lists (owner, 2026-11-21: no feature behind a shortcut alone;
 * `coverage.test.ts` holds it, RB-094-20).
 */
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
