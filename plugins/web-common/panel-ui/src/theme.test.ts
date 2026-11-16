import { describe, expect, it } from 'vitest';
import { applyTheme, themeFollower, themeOf } from './theme';

describe('the panel theme', () => {
  it('reads ui.theme, system for anything else', () => {
    expect(themeOf({ 'ui.theme': 'dark' })).toBe('dark');
    expect(themeOf({ 'ui.theme': 'light' })).toBe('light');
    expect(themeOf({ 'ui.theme': 'purple' })).toBe('system');
    expect(themeOf(undefined)).toBe('system');
  });

  it('sets the dark class, the colour scheme and data-theme', () => {
    const root = document.createElement('html');
    applyTheme(root, 'dark', false);
    expect(root.classList.contains('dark')).toBe(true);
    expect(root.style.colorScheme).toBe('dark');
    expect(root.dataset.theme).toBe('dark');
    applyTheme(root, 'system', false);
    expect(root.classList.contains('dark')).toBe(false);
    expect(root.dataset.theme).toBeUndefined();
  });

  it('follows the system under system, live', () => {
    const root = document.createElement('html');
    const listeners: ((event: { matches: boolean }) => void)[] = [];
    const query = { matches: true, addEventListener: (_: 'change', listener: (event: { matches: boolean }) => void) => listeners.push(listener) };
    const apply = themeFollower(root, () => query);
    expect(root.classList.contains('dark')).toBe(true);
    apply({ 'ui.theme': 'light' });
    expect(root.classList.contains('dark')).toBe(false);
    apply({ 'ui.theme': 'system' });
    query.matches = false;
    listeners[0]?.({ matches: false });
    expect(root.classList.contains('dark')).toBe(false);
  });
});
