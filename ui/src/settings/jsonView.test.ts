import { describe, expect, it } from 'vitest';
import type { SettingValue } from '../ipc/protocol';
import { jsonHighlightOn, scopeDocument, scopeText } from './jsonView';

const values: SettingValue[] = [
  { key: 'sync.scroll', value: false, origin: 'local' },
  { key: 'sync.click', value: true, origin: 'project' },
  { key: 'ui.theme', value: 'dark', origin: 'user' },
  { key: 'logs.level', value: 'info', origin: 'default' },
];

describe('the JSON view', () => {
  it('shows what each scope contributes, nested by key', () => {
    expect(scopeDocument(values, 'local')).toEqual({ sync: { scroll: false } });
    expect(scopeDocument(values, 'project')).toEqual({ sync: { click: true } });
    expect(scopeDocument(values, 'user')).toEqual({ ui: { theme: 'dark' } });
    expect(scopeDocument(values, 'effective')).toEqual({
      sync: { scroll: false, click: true },
      ui: { theme: 'dark' },
      logs: { level: 'info' },
    });
    expect(scopeText(values, 'local')).toBe('{\n  "sync": {\n    "scroll": false\n  }\n}\n');
  });

  it('colours unless settings.jsonHighlight is false (RB-080-42)', () => {
    expect(jsonHighlightOn(values)).toBe(true);
    expect(
      jsonHighlightOn([...values, { key: 'settings.jsonHighlight', value: false, origin: 'user' }]),
    ).toBe(false);
    expect(
      jsonHighlightOn([...values, { key: 'settings.jsonHighlight', value: true, origin: 'default' }]),
    ).toBe(true);
  });

  it('colours only while ui.syntaxHighlight is on as well (RB-080-65)', () => {
    const off = { key: 'ui.syntaxHighlight', value: false, origin: 'user' } as const;
    expect(jsonHighlightOn([...values, off])).toBe(false);
    expect(
      jsonHighlightOn([...values, off, { key: 'settings.jsonHighlight', value: true, origin: 'default' }]),
    ).toBe(false);
    expect(jsonHighlightOn([...values, { key: 'ui.syntaxHighlight', value: true, origin: 'default' }])).toBe(true);
  });
});
