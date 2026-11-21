// One entry of the activity bar (RB-090-67) as the components draw it: the activity's icon, its
// state in words (`text`, the group's name and the wide window's label), the counter a narrow
// window shows instead (`short`), and its controls - each a command of the registry the bar
// runs as `runCommand`, the primary one (stop, cancel, play, resync, real time, reset) with its word. The toolbar's
// compact form (`ActivityIndicators.vue`) draws the same entry as icon, counter and the primary
// control.

import type { Activity, ClockActivity, ViewRate } from '../ipc/activity';
import type { ShellToCore } from '../ipc/protocol';
import { ICONS } from '../icons';
import { clock } from '../portal/portal';
import { isPresenting, linkSecondsLeft } from '../portal/presentation';
import { checkCounts, elapsedSeconds, flowCounts, runningSteps, videoSeconds, type ActivityInputs } from './activities';
import { forcedStatesEntry } from './forcedStatesEntry';
import { pluginsEntry } from './pluginsEntry';

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
  /** The text the click puts on the clipboard instead of sending `message` (the presentation's
   * link, RB-096-05): only the clicked page may write the clipboard, the core has none. */
  copy?: string;
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

/** A rate as a percentage: `0`, `10`, `2.5`. */
export function percent(rate: number): string {
  return String(Math.round(rate * 1000) / 10);
}

/** The clock's start as text: a wall time `2026-12-24 10:00`, an instant with `UTC`. */
export function clockStart(clock: ClockActivity): string {
  const text = clock.start.slice(0, 16).replace('T', ' ');
  return clock.local ? text : `${text} UTC`;
}

/** The views at each rate, the rates in the order their first view has. */
function byRate(animations: readonly ViewRate[], viewLabel: (view: number) => string): [string, string][] {
  const groups = new Map<number, string[]>();
  for (const { view, rate } of animations) {
    groups.set(rate, [...(groups.get(rate) ?? []), viewLabel(view)]);
  }
  return [...groups.entries()].map(([rate, views]): [string, string] => [percent(rate), views.join(', ')]);
}

/** The presentation over the internet (RB-096-05): the viewers and the link's time left, Copy
 * link, New link and End. It stands in the portal's entry while it runs. */
function presentationEntry(data: EntryData, t: Translate, fold: string): EntryView | null {
  const presentation = data.inputs.portal?.presentation;
  if (presentation === undefined) return null;
  const count = presentation.viewers;
  const left = linkSecondsLeft(presentation, data.nowMs);
  const live = presentation.phase === 'live';
  const text = !live
    ? t('shell.activity.portal.present_starting')
    : left > 0
      ? t('shell.activity.portal.present_status_link', { count, time: clock(left) })
      : t('shell.activity.portal.present_status', { count });
  const actions: EntryAction[] = [];
  const link = presentation.link?.url;
  if (live && link !== undefined && left > 0) {
    actions.push({
      ...command('copy', ICONS.activityCopyLink, t('shell.activity.portal.present_copy'), ''),
      copy: link,
    });
  }
  if (live) {
    actions.push(
      command('newLink', ICONS.activityNewLink, t('shell.activity.portal.present_new_link'), 'portal.present.new_link'),
    );
  }
  actions.push(
    command('end', ICONS.activityEndPresent, t('shell.activity.portal.present_stop'), 'portal.present.stop', {
      primary: true,
    }),
  );
  return {
    activity: 'portal',
    icon: ICONS.activityPresent,
    color: live ? 'warning' : 'primary',
    text,
    short: String(count),
    actions,
    closeLabel: fold,
  };
}

/** The views' clock and animation rates (RB-094-07): what is changed, Step while stopped, 100 % and Real time. */
function timeEntry(data: EntryData, t: Translate, fold: string): EntryView | null {
  const time = data.inputs.state?.timeControl;
  if (time === undefined) return null;
  const clock = time.clock;
  const animations = time.animations ?? [];
  const start = clock === undefined ? null : clockStart(clock);
  const clockText =
    clock === undefined || start === null
      ? null
      : clock.speed === 0
        ? t('shell.activity.time.clock_fixed', { start })
        : t('shell.activity.time.clock_running', { start, speed: clock.speed });
  const groups = byRate(animations, data.viewLabel).map(([rate, views]) =>
    t('shell.activity.time.rate_views', { percent: rate, views }),
  );
  const animationsText = groups.length > 0 ? t('shell.activity.time.animations', { groups: groups.join('; ') }) : null;
  const text =
    clockText !== null && animationsText !== null
      ? t('shell.activity.time.both', { clock: clockText, animations: animationsText })
      : (clockText ?? animationsText ?? '');
  const actions: EntryAction[] = [];
  if (animations.some((entry) => entry.rate === 0)) {
    actions.push({
      ...command('step', ICONS.activityStep, t('shell.activity.time.step'), 'overlays.animations'),
      message: { type: 'runCommand', id: 'overlays.animations', args: { target: 'all', step: true } },
    });
  }
  if (animations.length > 0) {
    actions.push({
      ...command('normal', ICONS.activityNormal, t('shell.activity.time.normal'), 'overlays.animations'),
      message: { type: 'runCommand', id: 'overlays.animations', args: { target: 'all', rate: 1 } },
      primary: clock === undefined,
    });
  }
  if (clock !== undefined) {
    actions.push({
      ...command('realTime', ICONS.activityRealTime, t('shell.activity.time.real_time'), 'emulation.clock'),
      message: { type: 'runCommand', id: 'emulation.clock', args: { reset: true } },
      primary: true,
    });
  }
  const first = animations[0];
  return {
    activity: 'time',
    icon: clock !== undefined ? ICONS.activityClock : ICONS.activityAnimations,
    color: 'warning',
    text,
    short: start !== null ? start.slice(11) : t('shell.activity.time.percent', { percent: percent(first?.rate ?? 1) }),
    actions,
    closeLabel: fold,
  };
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
      // RB-090-68: paused, the time stands still and the entry says so.
      const paused = video.pausedAtMs !== undefined;
      const time = clock(videoSeconds(video, data.nowMs));
      const views = video.views.map(data.viewLabel).join(', ');
      return {
        activity,
        icon: paused ? ICONS.recordPaused : ICONS.video,
        color: paused ? 'neutral' : 'error',
        text: t(paused ? 'shell.activity.video.paused' : 'shell.activity.video.status', { time, views }),
        short: time,
        actions: [
          command(
            'pause',
            paused ? ICONS.recordResume : ICONS.recordPaused,
            t(paused ? 'shell.activity.video.resume' : 'shell.activity.video.pause'),
            'capture.video.pause',
          ),
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
      const status =
        queued > 0 ? t('shell.activity.flows.status_queued', { ...counts, queued }) : t('shell.activity.flows.status', counts);
      // RB-090-68: the step each running view is at, "step n/m".
      const steps = runningSteps(data.inputs.flowRun);
      const stepTexts = steps.map(({ view, number, total, name }) =>
        total === null
          ? t('shell.activity.flows.step', { view: data.viewLabel(view), number, name })
          : t('shell.activity.flows.step_of', { view: data.viewLabel(view), number, total, name }),
      );
      const only = steps.length === 1 ? steps[0] : undefined;
      return {
        activity,
        icon: ICONS.flowsPanel,
        color: 'primary',
        text: stepTexts.length > 0 ? t('shell.activity.flows.with_steps', { status, steps: stepTexts.join('; ') }) : status,
        short:
          only === undefined
            ? `${counts.done}/${counts.total}`
            : only.total === null
              ? String(only.number)
              : `${only.number}/${only.total}`,
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
      if (isPresenting(portal?.presentation)) return presentationEntry(data, t, fold);
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
    case 'time':
      return timeEntry(data, t, fold);
    case 'plugins':
      return pluginsEntry(state?.pluginProgress ?? [], t, fold);
    case 'forcedStates':
      return forcedStatesEntry(state?.forcedStates ?? [], data.viewLabel, t, fold);
  }
}
