import { describe, expect, it } from 'vitest';
import { defaultTextOf, helpTextOf } from './help';
import type { SettingDef } from './schema';

const t = (key: string, named?: Record<string, unknown>) =>
  key === 'settings.help.default' ? `Default: ${String(named?.value)}` : `<${key}>`;

function def(schema: SettingDef['schema']): SettingDef {
  return {
    key: 'checks.lighthouse',
    category: 'checks',
    section: 'lighthouse',
    order: 1,
    scopeAllowed: ['project'],
    localOnly: false,
    protectedFromMcp: false,
    labelKey: 'settings.key.checks.lighthouse.label',
    descriptionKey: 'settings.key.checks.lighthouse.description',
    helpKey: 'settings.key.checks.lighthouse.help',
    schema,
  };
}

describe('the help of a setting (RB-080-20)', () => {
  it('shows the catalogue text and the default from the schema', () => {
    expect(helpTextOf(def({ type: 'integer', default: 60 }), t)).toBe(
      '<settings.key.checks.lighthouse.help> Default: 60',
    );
  });

  it('names an unset, an empty and a listless default in words', () => {
    expect(defaultTextOf({}, t)).toBe('<settings.value.unset>');
    expect(defaultTextOf({ default: null }, t)).toBe('<settings.value.unset>');
    expect(defaultTextOf({ default: '' }, t)).toBe('<settings.value.empty>');
    expect(defaultTextOf({ default: [] }, t)).toBe('<settings.value.none>');
    expect(defaultTextOf({ default: 'Europe/Berlin' }, t)).toBe('Europe/Berlin');
    expect(defaultTextOf({ default: ['wcag22aa'] }, t)).toBe('["wcag22aa"]');
    expect(defaultTextOf({ default: true }, t)).toBe('true');
  });
});
