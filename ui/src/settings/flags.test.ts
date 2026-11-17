import { describe, expect, it } from 'vitest';
import type { SettingValue } from '../ipc/protocol';
import { flagOf, SECTION_TABS_KEY, SHOW_KEYS_KEY } from './flags';

function values(list: SettingValue[]): ReadonlyMap<string, SettingValue> {
  return new Map(list.map((value) => [value.key, value]));
}

describe('the settings window switches', () => {
  it('reads a set boolean and falls back to the default otherwise', () => {
    const set = values([
      { key: SHOW_KEYS_KEY, value: true, origin: 'user' },
      { key: SECTION_TABS_KEY, value: 'yes', origin: 'user' },
    ]);
    expect(flagOf(set, SHOW_KEYS_KEY, false)).toBe(true);
    expect(flagOf(set, SECTION_TABS_KEY, true)).toBe(true);
    expect(flagOf(values([]), SHOW_KEYS_KEY, false)).toBe(false);
  });
});
