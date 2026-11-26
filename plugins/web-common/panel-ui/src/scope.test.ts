import { describe, expect, it } from 'vitest';
import { memoryStorage } from './storage';
import { allViews, choiceOf, keepKnown, recallScope, rememberScope, scopeOf, withScope } from './scope';

describe('the scope of a test run (RB-099-10)', () => {
  it('runs every view, the visible views or the chosen breakpoints', () => {
    expect(scopeOf(allViews())).toBe('all');
    expect(scopeOf({ mode: 'visible', breakpoints: ['mobile'] })).toBe('visible');
    expect(scopeOf({ mode: 'chosen', breakpoints: ['mobile', 'desktop'] })).toEqual(['mobile', 'desktop']);
    expect(scopeOf({ mode: 'chosen', breakpoints: [] })).toBeNull();
  });

  it('sends a run over every view as it always was', () => {
    expect(withScope({ name: 'login' }, allViews())).toEqual({ name: 'login' });
    expect(withScope({ name: 'login' }, { mode: 'visible', breakpoints: [] })).toEqual({ name: 'login', scope: 'visible' });
    expect(withScope({}, { mode: 'chosen', breakpoints: ['mobile'] }, '$scope')).toEqual({ $scope: ['mobile'] });
    expect(withScope({}, { mode: 'chosen', breakpoints: [] })).toEqual({});
  });

  it('reads a stored scope or a summary back, and anything else as every view', () => {
    expect(choiceOf('visible')).toEqual({ mode: 'visible', breakpoints: [] });
    expect(choiceOf(['mobile', 'mobile', 'tablet'])).toEqual({ mode: 'chosen', breakpoints: ['mobile', 'tablet'] });
    for (const other of ['all', 'hidden', [], [''], [3], null, undefined, { mode: 'visible' }]) {
      expect(choiceOf(other)).toEqual(allViews());
    }
  });

  it('keeps only the breakpoints still there', () => {
    const chosen = { mode: 'chosen' as const, breakpoints: ['mobile', 'watch'] };
    expect(keepKnown(chosen, ['mobile', 'desktop'])).toEqual({ mode: 'chosen', breakpoints: ['mobile'] });
    expect(keepKnown(chosen, ['desktop'])).toEqual(allViews());
    expect(keepKnown({ mode: 'visible', breakpoints: [] }, [])).toEqual({ mode: 'visible', breakpoints: [] });
  });

  it('remembers the last choice per function, with no storage or a broken one as every view', () => {
    const storage = memoryStorage();
    expect(recallScope(storage, 'checks')).toEqual(allViews());
    rememberScope(storage, 'checks', { mode: 'chosen', breakpoints: ['mobile'] });
    rememberScope(storage, 'flows', { mode: 'visible', breakpoints: [] });
    expect(recallScope(storage, 'checks')).toEqual({ mode: 'chosen', breakpoints: ['mobile'] });
    expect(recallScope(storage, 'flows')).toEqual({ mode: 'visible', breakpoints: [] });
    expect(recallScope(null, 'checks')).toEqual(allViews());
    storage.setItem('rb.scope.visual', '{not json');
    expect(recallScope(storage, 'visual')).toEqual(allViews());
    const refusing = {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    };
    expect(() => rememberScope(refusing, 'checks', allViews())).not.toThrow();
    expect(recallScope(refusing, 'checks')).toEqual(allViews());
  });
});
