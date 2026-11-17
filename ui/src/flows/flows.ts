// The Flows panel's words (RB-090-60): a view's status as a colour and an icon, the views of a
// run by their labels, and where a failed view broke (its step and line).

import type { FlowViewResult, ViewInfo } from '../ipc/protocol';
import { RECORDER_ICONS } from '../recorder/icons';

export type StatusColor = 'success' | 'error' | 'warning' | 'neutral' | 'info';

/** A status the runner or `result.json` names: its colour and icon. */
export function statusLook(status: string): { color: StatusColor; icon: string } {
  switch (status) {
    case 'passed':
      return { color: 'success', icon: RECORDER_ICONS.flowPassed };
    case 'failed':
      return { color: 'error', icon: RECORDER_ICONS.flowFailed };
    case 'timedOut':
      return { color: 'warning', icon: RECORDER_ICONS.flowFailed };
    case 'skipped':
      return { color: 'neutral', icon: RECORDER_ICONS.flowSkipped };
    case 'queued':
      return { color: 'neutral', icon: RECORDER_ICONS.flowQueued };
    default:
      return { color: 'info', icon: RECORDER_ICONS.flowRunning };
  }
}

/** The label of view `id`, its number when the grid has it no more. */
export function viewLabel(views: readonly ViewInfo[], id: number): string {
  return views.find((view) => view.id === id)?.label ?? `#${id}`;
}

/** Where a failed view broke: `step` and `line` when the result has them. */
export function failurePlace(view: FlowViewResult): { step: string | null; line: number | null } {
  return { step: view.error?.step ?? null, line: view.error?.line ?? null };
}
