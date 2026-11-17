// The activity bar's part of the protocol (RB-090-67), mirror of rb-ipc's `activity.rs`; kept
// beside `protocol.ts`, which re-exports it. The snapshot names where the core made room for the
// bar, the entries folded into their toolbar indicator, the recording just stopped and the
// running video; the shell asks for the room (`activityBar`) and folds entries
// (`collapseActivity`).

import type { ViewId } from './protocol';

/** An activity with an entry in the bar, in the bar's order. */
export type Activity = 'video' | 'recorder' | 'flows' | 'checks' | 'portal' | 'desync';

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
  /** It stops by itself after this many seconds. */
  maxDurationSec: number;
}

/** The activity bar's fields of the snapshot. */
export interface ActivitySnapshot {
  /** The running video recording; absent while none runs. */
  video?: VideoActivity;
  activityBar?: ActivityBarState;
}

/** The shell's requests: room for the bar at `place` (none gives it back); an entry folded. `ack`. */
export type ActivityRequest =
  | { type: 'activityBar'; place?: ActivityBarPosition }
  | { type: 'collapseActivity'; activity: Activity; collapsed: boolean };
