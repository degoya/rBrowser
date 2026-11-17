// What the shell shows while DevTools are open (RB-010-06): which view they inspect and where.
// The core's snapshot carries `devtools: { view, mode } | null`; the label names the view by its
// cell label, falling back to its id when the view is gone.

import type { DevtoolsMode, StateSnapshot } from '../ipc/protocol';

/** The i18n key of each DevTools mode. */
export const MODE_KEYS: Record<DevtoolsMode, string> = {
  'docked-bottom': 'shell.devtools.mode.docked_bottom',
  'docked-right': 'shell.devtools.mode.docked_right',
  window: 'shell.devtools.mode.window',
};

export interface DevtoolsIndicator {
  /** The inspected view's name. */
  view: string;
  /** The i18n key of where DevTools are. */
  modeKey: string;
}

/** The indicator for `state`; `null` while DevTools are closed. */
export function devtoolsIndicator(state: StateSnapshot | null): DevtoolsIndicator | null {
  const devtools = state?.devtools ?? null;
  if (devtools === null) {
    return null;
  }
  const view = state?.views.find((v) => v.id === devtools.view);
  return {
    view: view?.label ?? `#${devtools.view}`,
    modeKey: MODE_KEYS[devtools.mode],
  };
}
