// One entry of the activity bar (RB-090-67) as the components draw it: the activity's icon, its
// state in words (`text`, the group's name and the wide window's label), the counter a narrow
// window shows instead (`short`), and its controls - each a command of the registry the bar
// runs as `runCommand`, the primary one (stop, cancel, play, resync) with its word. The toolbar's
// compact form (`ActivityIndicators.vue`) draws the same entry as icon, counter and the primary
// control.

import type { Activity } from '../ipc/activity';
import type { ShellToCore } from '../ipc/protocol';
import { ICONS } from '../icons';
import { clock } from '../portal/portal';
import { checkCounts, elapsedSeconds, flowCounts, type ActivityInputs } from './activities';

/** `t` of vue-i18n, as far as the entries need it. */
export type Translate = (key: string, args?: Record<string, unknown>) => string;

/** One control of an entry. */
export interface EntryAction {
  /** For the tests and the key in the list. */
  key: string;
  icon: string;
  /** Its name; the primary control shows it as its word. */
  label: string;
  /** The registry command, for the shortcut in its tooltip. */
  command: string;
  message: ShellToCore;
  /** The entry's main control: stop, cancel, play, resync. */
  primary?: boolean;
  /** Running it ends the entry (Play last closes the offer). */
  closes?: boolean;
}

/** One entry. */
export interface EntryView {
  activity: Activity;
  icon: string;
  /** The status colour of the icon. */
  color: 'error' | 'primary' | 'warning' | 'neutral';
  /** The state in words. */
  text: string;
  /** The counter a narrow window shows. */
  short: string;
  actions: EntryAction[];
  /** The close button's name: fold into the toolbar, or end the offer to play. */
  closeLabel: string;
}

/** What the entries are drawn from. */
export interface EntryData {
  inputs: ActivityInputs;
  nowMs: number;
  /** A view's label for its id. */
  viewLabel: (view: number) => string;
}

function command(key: string, icon: string, label: string, id: string, extra: Partial<EntryAction> = {}): EntryAction {
  return { key, icon, label, command: id, message: { type: 'runCommand', id }, ...extra };
}

/** The entry of `activity`; `null` while it does not run. */
export function entryView(activity: Activity, data: EntryData, t: Translate): EntryView | null {
  const name = t(`shell.activity.name.${activity}`);
  const fold = t('shell.activity.close', { name });
  const { state } = data.inputs;
  switch (activity) {
    case 'video': {
      const video = state?.video;
      if (video === undefined) return null;
      const time = clock(elapsedSeconds(video.startedAtMs, data.nowMs));
      const views = video.views.map(data.viewLabel).join(', ');
      return {
        activity,
        icon: ICONS.video,
        color: 'error',
        text: t('shell.activity.video.status', { time, views }),
        short: time,
        actions: [
          command('stop', ICONS.videoStop, t('shell.activity.video.stop'), 'capture.video.toggle', { primary: true }),
        ],
        closeLabel: fold,
      };
    }
    case 'recorder': {
      const recorder = state?.recorder;
      if (recorder === undefined) {
        const recorded = state?.activityBar?.recorded;
        if (recorded === undefined) return null;
        const play = t('shell.activity.recorder.play');
        return {
          activity,
          icon: ICONS.record,
          color: 'neutral',
          text: t('shell.activity.recorder.recorded', { name: recorded }),
          short: recorded,
          actions: [command('play', ICONS.recordResume, play, 'recorder.play', { primary: true, closes: true })],
          closeLabel: t('shell.activity.recorder.dismiss', { name: recorded }),
        };
      }
      const paused = recorder.phase === 'paused';
      const args = { name: recorder.name, steps: recorder.steps };
      return {
        activity,
        icon: paused ? ICONS.recordPaused : ICONS.record,
        color: paused ? 'neutral' : 'error',
        text: t(paused ? 'shell.activity.recorder.paused' : 'shell.activity.recorder.recording', args),
        short: String(recorder.steps),
        actions: [
          command(
            'pause',
            paused ? ICONS.recordResume : ICONS.recordPaused,
            t(paused ? 'shell.activity.recorder.resume' : 'shell.activity.recorder.pause'),
            'recorder.pause',
          ),
          command('check', ICONS.activityCheck, t('shell.activity.recorder.check'), 'recorder.check'),
          command('stop', ICONS.recordStop, t('shell.activity.recorder.stop'), 'recorder.stop', { primary: true }),
        ],
        closeLabel: fold,
      };
    }
    case 'flows': {
      const counts = flowCounts(data.inputs.flowRun);
      if (counts === null) return null;
      // The flows of "Run all" still waiting: without them a queue reads as one flow looping.
      const queued = data.inputs.flowRun?.running?.queued?.length ?? 0;
      return {
        activity,
        icon: ICONS.flowsPanel,
        color: 'primary',
        text:
          queued > 0
            ? t('shell.activity.flows.status_queued', { ...counts, queued })
            : t('shell.activity.flows.status', counts),
        short: `${counts.done}/${counts.total}`,
        actions: [
          command('results', ICONS.flowReveal, t('shell.activity.flows.results'), 'flows.open_results'),
          command('cancel', ICONS.flowCancel, t('shell.activity.flows.stop'), 'flows.cancel', { primary: true }),
        ],
        // The close button only folds the entry; the run goes on until Stop or its end.
        closeLabel: t('shell.activity.flows.fold'),
      };
    }
    case 'checks': {
      const progress = data.inputs.checks;
      const counts = checkCounts(progress);
      if (counts === null || progress?.finished === true) return null;
      return {
        activity,
        icon: ICONS.checks,
        color: 'primary',
        text: t('shell.activity.checks.status', counts),
        short: `${counts.done}/${counts.total}`,
        actions: [
          command('cancel', ICONS.flowCancel, t('shell.activity.checks.cancel'), 'checks.cancel', { primary: true }),
        ],
        closeLabel: fold,
      };
    }
    case 'portal': {
      const portal = data.inputs.portal;
      if (portal?.running !== true) return null;
      const count = portal.devices.length;
      return {
        activity,
        icon: ICONS.portal,
        color: 'primary',
        text: t('shell.activity.portal.status', { count }),
        short: String(count),
        actions: [
          command('devices', ICONS.portal, t('shell.activity.portal.devices', { count }), 'portal.devices'),
          command('qr', ICONS.activityQr, t('shell.activity.portal.qr'), 'portal.show_qr'),
          {
            ...command('disconnect', ICONS.activityDisconnect, t('shell.activity.portal.disconnect'), 'portal.toggle'),
            message: { type: 'runCommand', id: 'portal.toggle', args: { enabled: false } },
            primary: true,
          },
        ],
        closeLabel: fold,
      };
    }
    case 'desync': {
      const desynced = state?.sync.desynced ?? [];
      if (desynced.length === 0) return null;
      const views = desynced.map((entry) => data.viewLabel(entry.view)).join(', ');
      return {
        activity,
        icon: ICONS.activityDesync,
        color: 'warning',
        text: t('shell.activity.desync.status', { count: desynced.length, views }),
        short: String(desynced.length),
        actions: [
          command('resync', ICONS.activityResync, t('shell.activity.desync.resync'), 'sync.resync', { primary: true }),
        ],
        closeLabel: fold,
      };
    }
  }
}
