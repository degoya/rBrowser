import { describe, expect, it } from 'vitest';
import { InpTracker } from './inp';

describe('InpTracker', () => {
  it('has no value before an interaction', () => {
    expect(new InpTracker().value()).toBeNull();
  });

  it('takes the longest entry of one interaction', () => {
    const inp = new InpTracker();
    inp.add({ interactionId: 7, duration: 40, target: '#button' });
    inp.add({ interactionId: 7, duration: 320, target: '#button' });
    inp.add({ interactionId: 7, duration: 56 });
    expect(inp.value()).toEqual({ value: 320, target: '#button', interactions: 1 });
  });

  it('is the longest interaction below fifty interactions', () => {
    const inp = new InpTracker();
    inp.add({ interactionId: 1, duration: 80 });
    inp.add({ interactionId: 2, duration: 300, target: '#slow' });
    inp.add({ interactionId: 3, duration: 120 });
    expect(inp.value()).toEqual({ value: 300, target: '#slow', interactions: 3 });
  });

  it('drops one outlier per fifty interactions (the 98th percentile)', () => {
    const inp = new InpTracker();
    inp.add({ interactionId: 1, duration: 900, target: '#outlier' });
    for (let id = 2; id <= 50; id += 1) inp.add({ interactionId: id, duration: 100 });
    inp.add({ interactionId: 51, duration: 250, target: '#second' });
    expect(inp.value()).toEqual({ value: 250, target: '#second', interactions: 51 });
  });

  it('counts interactions the page reports when it knows more than it saw', () => {
    const inp = new InpTracker();
    inp.add({ interactionId: 1, duration: 900, target: '#outlier' });
    inp.add({ interactionId: 2, duration: 200, target: '#b' });
    expect(inp.value(60)).toEqual({ value: 200, target: '#b', interactions: 60 });
  });

  it('ignores entries without an interaction', () => {
    const inp = new InpTracker();
    inp.add({ interactionId: 0, duration: 800 });
    expect(inp.value()).toBeNull();
  });

  it('starts over on reset', () => {
    const inp = new InpTracker();
    inp.add({ interactionId: 1, duration: 300 });
    inp.reset();
    expect(inp.value()).toBeNull();
  });
});
