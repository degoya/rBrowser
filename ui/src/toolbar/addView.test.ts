import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import { hiddenBreakpoints } from './addView';

describe('the hidden breakpoints the "+" counts', () => {
  it('are listed by width', () => {
    const state = sampleState();
    state.layout.hidden = ['desktop', 'mobile'];
    expect(hiddenBreakpoints(state).map((b) => b.id)).toEqual(['mobile', 'desktop']);
    expect(hiddenBreakpoints(sampleState())).toEqual([]);
    expect(hiddenBreakpoints(null)).toEqual([]);
  });
});
