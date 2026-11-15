import { describe, expect, it } from 'vitest';
import { badgeOf, defaultScope, scopeChoices } from './scope';
import { realDefs } from './test/fakeCore';

const defs = new Map(realDefs().map((def) => [def.key, def]));
const certificates = defs.get('network.ignoreCertificateErrors')!;
const scroll = defs.get('sync.scroll')!;
const theme = defs.get('ui.theme')!;

describe('scopes at a field', () => {
  it('badges every origin', () => {
    expect(badgeOf('local').labelKey).toBe('settings.origin.local');
    expect(badgeOf('project').labelKey).toBe('settings.origin.project');
    expect(badgeOf('user').labelKey).toBe('settings.origin.user');
    expect(badgeOf('default').labelKey).toBe('settings.origin.default');
  });

  it('locks the project for a local-only key and names the reason', () => {
    const choices = scopeChoices(certificates, true);
    expect(choices.find((c) => c.scope === 'project')?.reasonKey).toBe('settings.scope.local_only_reason');
    expect(choices.find((c) => c.scope === 'local')?.reasonKey).toBeUndefined();
    expect(defaultScope(certificates, 'default', true)).toBe('local');
  });

  it('saves where the value is stored, and nowhere without a project', () => {
    expect(defaultScope(scroll, 'local', true)).toBe('local');
    expect(defaultScope(scroll, 'default', true)).toBe('project');
    expect(defaultScope(scroll, 'default', false)).toBeNull();
    expect(scopeChoices(scroll, false).every((c) => c.reasonKey === 'settings.scope.no_project')).toBe(true);
    expect(defaultScope(theme, 'default', false)).toBe('user');
  });
});
