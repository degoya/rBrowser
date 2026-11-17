import { describe, expect, it } from 'vitest';
import { PACE_KEY, PACES, paceOf } from './pace';

describe('the playback pace (RB-090-66)', () => {
  it('is fast or recorded, fast for anything else', () => {
    expect(PACES).toEqual(['fast', 'recorded']);
    expect(paceOf('recorded')).toBe('recorded');
    expect(paceOf('fast')).toBe('fast');
    expect(paceOf(undefined)).toBe('fast');
    expect(paceOf('slow')).toBe('fast');
  });

  it('is the recorder setting', () => {
    expect(PACE_KEY).toBe('automation.recorder.playbackPace');
  });
});
