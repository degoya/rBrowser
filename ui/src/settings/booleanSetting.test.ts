import { describe, expect, it } from 'vitest';
import { booleanSetting } from './booleanSetting';

describe('a boolean setting', () => {
  it('is the value the core sent, the default for anything else', () => {
    expect(booleanSetting(false, true)).toBe(false);
    expect(booleanSetting(true, false)).toBe(true);
    for (const odd of [undefined, null, 'false', 0, {}]) {
      expect(booleanSetting(odd, true)).toBe(true);
      expect(booleanSetting(odd, false)).toBe(false);
    }
  });
});
