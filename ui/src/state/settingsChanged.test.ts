import { describe, expect, it } from 'vitest';
import { changedValues, touches } from './settingsChanged';

const theme = { key: 'ui.theme', value: 'dark', origin: 'user' } as const;

describe('a settingsChanged event (RB-090-36)', () => {
  it('gives the new values by key when it carries one for every key', () => {
    const changed = changedValues({ keys: ['ui.theme'], values: [theme] });
    expect(changed?.get('ui.theme')).toEqual(theme);
  });

  it('gives nothing - the page asks - without values or with a key left out', () => {
    expect(changedValues({ keys: ['ui.theme'] })).toBeNull();
    expect(changedValues({ keys: ['ui.theme', 'plugins.x.y'], values: [theme] })).toBeNull();
  });

  it('says whether it names a key a page reads', () => {
    expect(touches({ keys: ['ui.theme', 'sync.scroll'] }, ['sync.scroll'])).toBe(true);
    expect(touches({ keys: ['ui.theme'] }, ['sync.scroll'])).toBe(false);
  });
});
