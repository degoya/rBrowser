// The activity bar's part of the protocol (RB-090-67), mirror of rb-ipc's `activity.rs` (and
// `time_control.rs`, RB-094-07, `plugin_progress.rs`, RB-093-01, and `forced_states.rs`,
// RB-094-16); kept beside `protocol.ts`, which re-exports it. The snapshot names where the core
// made room for the bar, the entries folded into their toolbar indicator, the recording just
// stopped, the running video, the changed clock, the plugins' progress and the forced
// pseudo-class states; the shell asks for the room (`activityBar`) and folds entries
// (`collapseActivity`).

import type { ViewId } from './protocol';

/** An activity with an entry in the bar, in the bar's order. */
export type Activity =
  | 'video'
  | 'recorder'
  | 'flows'
  | 'checks'
  | 'portal'
  | 'desync'
  | 'time'
  | 'plugins'
  | 'forcedStates';

/** `ui.activityBar.position`. */
export type ActivityBarPosition = 'top' | 'bottom';

/** The activity bar in the snapshot; absent while it tells nothing. */
export interface ActivityBarState {
  /** Where the core made room for the bar: the page there draws it. */
  place?: ActivityBarPosition;
  /** The entries folded into their toolbar indicator until their activity ends. */
  collapsed?: Activity[];
  /** The recording that just stopped, offered for "Play last". */
  recorded?: string;
}

/** A video recording that runs. */
export interface VideoActivity {
  views: ViewId[];
  /** When it started, ms since the epoch. */
  startedAtMs: number;
  /** It stops by itself after this many seconds (of recorded time). */
  maxDurationSec: number;
  /** While paused (RB-090-68): when the pause began, ms since the epoch; the time stands still. */
  pausedAtMs?: number;
  /** The ms of the pauses that ended: the time leaves them out. */
  pausedMs?: number;
}

/** The views' clock (RB-094-07): one for every view. */
export interface ClockActivity {
  /** Where it started: `2026-12-24T10:00:00` (each page's local time, `local`) or RFC 3339 in UTC. */
  start: string;
  local: boolean;
  /** 0 fixed, 1 real time, more is faster. */
  speed: number;
}

/** A view's animation rate: 0 stopped, 1 normal. */
export interface ViewRate {
  view: ViewId;
  rate: number;
}

/** The clock and the animation rates while either is changed (RB-094-07). */
export interface TimeControlActivity {
  clock?: ClockActivity;
  /** The views not at 100 %, in view order. */
  animations?: ViewRate[];
}

/** A plugin call that reports its progress (rb-ipc's `plugin_progress.rs`, RB-093-01). */
export interface PluginProgress {
  /** The plugin id. */
  plugin: string;
  /** Its display name. */
  name: string;
  /** Steps done, never past `total`. */
  done: number;
  /** Steps in all; 0 while the plugin does not know. */
  total: number;
  /** The registry command that cancels the call; absent when the plugin has none. */
  cancel?: string;
}

/** One view's forced pseudo-class states (rb-ipc's `forced_states.rs`, RB-094-16). */
export interface ViewForcedStates {
  view: ViewId;
  /** CDP's words for the states (`hover`, `focus-within`), in the page menu's order. */
  states: string[];
}

/** The activity bar's fields of the snapshot. */
export interface ActivitySnapshot {
  /** The running video recording; absent while none runs. */
  video?: VideoActivity;
  activityBar?: ActivityBarState;
  /** The views' clock and animation rates; absent while the time is real and all run at 100 %. */
  timeControl?: TimeControlActivity;
  /** The plugin calls that report their progress, by plugin id; absent while none does. */
  pluginProgress?: PluginProgress[];
  /** The views with forced pseudo-class states; absent while none has one (RB-094-16). */
  forcedStates?: ViewForcedStates[];
}

/** The shell's requests: room for the bar at `place` (none gives it back); an entry folded. `ack`. */
export type ActivityRequest =
  | { type: 'activityBar'; place?: ActivityBarPosition }
  | { type: 'collapseActivity'; activity: Activity; collapsed: boolean };
