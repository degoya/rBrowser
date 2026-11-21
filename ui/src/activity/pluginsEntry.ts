// The activity bar's plugins entry (RB-093-01): the plugin calls that report their progress
// (`host.report-progress`), as the core publishes them in the snapshot's `pluginProgress` -
// throttled, and gone when the plugin's last call ended. The entry names the first plugin and how
// far it is (the others counted), and offers each plugin's own cancel command where its manifest
// has one (the width sweep's `plugin.width_sweep.cancel`).

import type { PluginProgress } from '../ipc/activity';
import { ICONS } from '../icons';
import type { EntryAction, EntryView, Translate } from './entries';

function status(progress: PluginProgress, t: Translate): string {
  const { name, done, total } = progress;
  return total > 0 ? t('shell.activity.plugins.status', { name, done, total }) : t('shell.activity.plugins.working', { name });
}

function cancel(progress: PluginProgress, command: string, t: Translate, primary: boolean): EntryAction {
  return {
    key: `cancel-${progress.plugin}`,
    icon: ICONS.flowCancel,
    label: t('shell.activity.plugins.cancel', { name: progress.name }),
    command,
    message: { type: 'runCommand', id: command },
    primary,
  };
}

/** The entry of `progress`; `null` while no plugin call reports any. `fold` names the close button. */
export function pluginsEntry(progress: readonly PluginProgress[], t: Translate, fold: string): EntryView | null {
  const [first] = progress;
  if (first === undefined) return null;
  const own = status(first, t);
  const actions = progress.flatMap((entry) => (entry.cancel === undefined ? [] : [cancel(entry, entry.cancel, t, false)]));
  const [main] = actions;
  if (main !== undefined) main.primary = true;
  return {
    activity: 'plugins',
    icon: ICONS.activityPlugin,
    color: 'primary',
    text: progress.length > 1 ? t('shell.activity.plugins.more', { status: own, count: progress.length - 1 }) : own,
    short: first.total > 0 ? `${first.done}/${first.total}` : String(first.done),
    actions,
    closeLabel: fold,
  };
}
