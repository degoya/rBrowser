// The activity bar's forced states entry (RB-094-16): the pseudo-class states forced on an
// element of a view (`view.force_state.set`, the page menu's Force state), as the core publishes
// them in the snapshot's `forcedStates`. The entry names the states and the views - the views
// with the same states together - counts the views in a narrow window, and resets every view
// with the existing command (`view.force_state.reset`). It ends when the last state goes: a
// reset, a navigation or the view closed.

import type { ViewForcedStates } from '../ipc/activity';
import { ICONS } from '../icons';
import type { EntryView, Translate } from './entries';

/** The views of each set of states, the sets in the order their first view has. */
function groups(forced: readonly ViewForcedStates[], viewLabel: (view: number) => string): [string, string][] {
  const byStates = new Map<string, string[]>();
  for (const { view, states } of forced) {
    const key = states.map((state) => `:${state}`).join(', ');
    byStates.set(key, [...(byStates.get(key) ?? []), viewLabel(view)]);
  }
  return [...byStates.entries()].map(([states, views]): [string, string] => [states, views.join(', ')]);
}

/** The entry of `forced`; `null` while no view has a forced state. `fold` names the close button. */
export function forcedStatesEntry(
  forced: readonly ViewForcedStates[],
  viewLabel: (view: number) => string,
  t: Translate,
  fold: string,
): EntryView | null {
  if (forced.length === 0) return null;
  const text = groups(forced, viewLabel)
    .map(([states, views]) => t('shell.activity.forced_states.group', { states, views }))
    .join('; ');
  return {
    activity: 'forcedStates',
    icon: ICONS.activityForced,
    color: 'warning',
    text: t('shell.activity.forced_states.status', { groups: text }),
    short: String(forced.length),
    actions: [
      {
        key: 'reset',
        icon: ICONS.activityResetStates,
        label: t('shell.activity.forced_states.reset'),
        command: 'view.force_state.reset',
        message: { type: 'runCommand', id: 'view.force_state.reset', args: { target: 'all' } },
        primary: true,
      },
    ],
    closeLabel: fold,
  };
}
