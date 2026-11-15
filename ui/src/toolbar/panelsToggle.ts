// The plugin panels' switch in the toolbar (RB-080-79, `PanelsToggle.vue`): the command it runs and
// the settings it reads.

/** Opens the panels or folds them to their icon rail (`Leader P Z`, RB-080-61, RB-080-93). */
export const PANELS_COMMAND = 'panels.dock.toggle';
/** Where the panels are: the switch shows that side's icon. */
export const PANELS_DOCK_SETTING = 'panels.dock';
/** `open`, `rail` or `hidden`: the switch is on while they are open. */
export const PANELS_STATE_SETTING = 'panels.dockState';
/** Whether the toolbar shows the switch; off by default (RB-090-10). */
export const PANELS_TOGGLE_SETTING = 'ui.toolbar.panelsToggle';
