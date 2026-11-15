import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import type { ViewInfo } from '../ipc/protocol';
import { assignMessage, badgeOf, clearMessage, sessionsOf } from './sessions';

const view = (session: string): ViewInfo => ({ ...sampleState().views[0]!, session });

describe('sessions', () => {
  it('lists default first, also when the core sent none', () => {
    expect(sessionsOf(sampleState()).map((s) => s.id)).toEqual(['default', 'admin']);
    expect(sessionsOf({ ...sampleState(), sessions: [] }).map((s) => s.id)).toEqual(['default']);
    expect(sessionsOf(null).map((s) => s.id)).toEqual(['default']);
  });

  it('gives a named session a badge in its colour and default none', () => {
    const sessions = sessionsOf(sampleState());
    expect(badgeOf(view('default'), sessions)).toBeNull();
    expect(badgeOf(view('admin'), sessions)).toEqual({ name: 'admin', color: '#e5484d' });
    expect(badgeOf(view('ghost'), sessions)).toEqual({ name: 'ghost', color: null });
  });

  it('runs the commands with their arguments', () => {
    expect(assignMessage(3, 'admin')).toEqual({
      type: 'runCommand',
      id: 'session.assign',
      args: { target: 3, session: 'admin' },
    });
    expect(clearMessage('admin')).toEqual({
      type: 'runCommand',
      id: 'session.clear',
      args: { session: 'admin' },
    });
  });
});
