// RB-099-04: rBrowser's sign-in dialog - the questions the core asks, one per origin, realm and
// proxy, and the one answer to each.
import { describe, expect, it } from 'vitest';
import type { AuthPrompt, CoreToShell, ViewInfo } from '../ipc/protocol';
import { nextQueue, problemField, replyMessage, signInProblem, viewNames } from './authPrompt';

const SITE: AuthPrompt = {
  id: 1,
  origin: 'https://other.example.test:443',
  realm: 'Members',
  proxy: false,
  views: [1],
  refused: false,
  insecure: false,
};
const PROXY: AuthPrompt = {
  id: 2,
  origin: 'proxy.example.test:3128',
  proxy: true,
  views: [2],
  refused: false,
  insecure: false,
};

describe('the sign-in questions', () => {
  it('queues a new question and updates an open one in place', () => {
    const one = nextQueue([], { type: 'authPrompt', ...SITE });
    expect(one).toEqual([SITE]);
    const two = nextQueue(one ?? [], { type: 'authPrompt', ...PROXY });
    expect(two?.map((prompt) => prompt.id)).toEqual([1, 2]);
    const more = nextQueue(two ?? [], { type: 'authPrompt', ...SITE, views: [1, 3] });
    expect(more?.map((prompt) => prompt.id)).toEqual([1, 2]);
    expect(more?.[0]?.views).toEqual([1, 3]);
  });

  it('drops a withdrawn question and ignores every other message', () => {
    expect(nextQueue([SITE, PROXY], { type: 'authPromptClosed', id: 1 })).toEqual([PROXY]);
    expect(nextQueue([SITE], { type: 'authPromptClosed', id: 9 })).toEqual([SITE]);
    const other: CoreToShell = { type: 'ack' };
    expect(nextQueue([SITE], other)).toBeNull();
  });

  it('answers with the credential once, or cancels without one', () => {
    expect(replyMessage(1, { user: 'ada', password: 'pw-12345', save: false })).toEqual({
      type: 'authPromptReply',
      id: 1,
      signIn: { user: 'ada', password: 'pw-12345', save: false },
    });
    expect(replyMessage(1)).toEqual({ type: 'authPromptReply', id: 1 });
  });

  it('checks a credential to save as the store does, and asks nothing of a one-time answer', () => {
    expect(signInProblem({ user: 'ada', password: 'pw', save: true })).toBe('auth.bar.problem.passwordShort');
    expect(signInProblem({ user: 'a:b', password: 'pw-12345', save: true })).toBe('auth.bar.problem.user');
    expect(signInProblem({ user: 'ada', password: 'pw-12345', save: true })).toBeNull();
    expect(signInProblem({ user: '', password: '', save: false })).toBeNull();
    expect(signInProblem({ user: 'ada', password: 'line\nbreak', save: false })).toBe('auth.bar.problem.password');
  });

  it('names the field a problem is about, so that field is marked invalid (RB-099-21)', () => {
    expect(problemField('auth.bar.problem.user')).toBe('user');
    expect(problemField('auth.bar.problem.password')).toBe('password');
    expect(problemField('auth.bar.problem.passwordShort')).toBe('password');
    expect(problemField('auth.bar.problem.environment')).toBeNull();
    expect(problemField(null)).toBeNull();
  });

  it('names the views waiting by their labels', () => {
    const views = [
      { id: 1, label: 'Desktop' },
      { id: 3, label: 'Mobile' },
    ] as ViewInfo[];
    expect(viewNames([1, 3], views)).toBe('Desktop, Mobile');
    expect(viewNames([1, 7], views)).toBe('Desktop, 7');
  });
});
