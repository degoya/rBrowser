// RB-090-68: the Video menu (the split button's and the main menu's) pauses the running recording
// - a check row, checked while paused, no choice without a recording.

import { describe, expect, it } from 'vitest';
import type { CommandInfo, NativeMenuRow, StateSnapshot } from '../ipc/protocol';
import { sampleState } from '../ipc/mockState';
import { VIDEO_MENU } from './buttonMenus';
import { buildMenu, type MenuContext } from './menuModel';

const PAUSE: CommandInfo = {
  id: 'capture.video.pause',
  titleKey: 'commands.capture.video.pause',
  group: 'capture',
  bindings: ['Ctrl+Shift+Space C Z'],
  defaultBindings: ['Ctrl+Shift+Space C Z'],
  changed: false,
  mcp: { covered: 'record_video' },
};

function pauseRow(video: StateSnapshot['video']): Extract<NativeMenuRow, { kind: 'item' }> | undefined {
  const state = { ...sampleState(), ...(video === undefined ? {} : { video }) };
  const ctx: MenuContext = { t: (key) => key, state, settings: {}, commands: [PAUSE] };
  const built = buildMenu(VIDEO_MENU, ctx);
  const row = built.rows.find(
    (candidate): candidate is Extract<NativeMenuRow, { kind: 'item' }> =>
      candidate.kind === 'item' && candidate.label === 'commands.capture.video.pause',
  );
  if (row !== undefined) {
    expect(built.actions.get(row.id)).toEqual({ type: 'runCommand', id: 'capture.video.pause' });
  }
  return row;
}

describe('the Video menu (RB-090-68)', () => {
  it('pauses a running recording and marks it while paused', () => {
    expect(pauseRow(undefined)).toMatchObject({ disabled: true, checked: false });
    const running = { views: [1], startedAtMs: 0, maxDurationSec: 600 };
    expect(pauseRow(running)).toMatchObject({ checked: false, accel: 'Ctrl+Shift+Space C Z' });
    expect(pauseRow(running)?.disabled).toBeUndefined();
    expect(pauseRow({ ...running, pausedAtMs: 5000 })).toMatchObject({ checked: true });
  });
});
