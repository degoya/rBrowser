import { describe, expect, it } from 'vitest';
import { shellMessages } from '../i18n';
import type { SettingValue } from '../ipc/protocol';
import { filterOf, matchFields, matches } from './search';
import { realDefs } from './test/fakeCore';

const catalogue = shellMessages();
function t(key: string): string {
  const found = key.split('.').reduce<unknown>((node, part) => (node as Record<string, unknown> | undefined)?.[part], catalogue);
  return typeof found === 'string' ? found : key;
}
const defs = realDefs();
const scroll = defs.find((def) => def.key === 'sync.scroll')!;

describe('the settings search', () => {
  it('finds sync.scroll by its label, its key and its description', () => {
    expect(matchFields(scroll, 'scroll', t)).toEqual(['label', 'key', 'description', 'help']);
    expect(matchFields(scroll, 'Sync scrolling', t)).toEqual(['label']);
    expect(matchFields(scroll, 'sync.scroll', t)).toEqual(['key']);
    expect(matchFields(scroll, 'in one view', t)).toEqual(['description']);
    expect(defs.filter((def) => matches(def, 'scroll', t)).map((def) => def.key)).toContain('sync.scroll');
  });

  it('needs every word and ignores case', () => {
    expect(matches(scroll, 'SYNC scroll', t)).toBe(true);
    expect(matches(scroll, 'scroll firefox', t)).toBe(false);
    expect(matches(scroll, '   ', t)).toBe(true);
  });

  it('keeps only the changed keys when asked', () => {
    const values = new Map<string, SettingValue>([
      ['sync.scroll', { key: 'sync.scroll', value: false, origin: 'local' }],
      ['sync.click', { key: 'sync.click', value: true, origin: 'default' }],
    ]);
    const keep = filterOf({ query: '', onlyChanged: true }, values, t);
    expect(defs.filter(keep).map((def) => def.key)).toEqual(['sync.scroll']);
  });
});
