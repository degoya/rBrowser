import { describe, expect, it } from 'vitest';
import { applyTheme, nextTheme, parseTheme, resolveTheme, watchSystemDark } from './theme';

describe('the theme', () => {
  it('reads the ui.theme setting, system by default', () => {
    expect(parseTheme('dark')).toBe('dark');
    expect(parseTheme('light')).toBe('light');
    expect(parseTheme('system')).toBe('system');
    expect(parseTheme('purple')).toBe('system');
    expect(parseTheme(null)).toBe('system');
  });

  it('follows the system only when asked', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });

  it('sets the dark class and the colour scheme on the root', () => {
    const root = document.createElement('html');
    applyTheme(root, 'dark');
    expect(root.classList.contains('dark')).toBe(true);
    expect(root.style.colorScheme).toBe('dark');
    applyTheme(root, 'light');
    expect(root.classList.contains('dark')).toBe(false);
    expect(root.style.colorScheme).toBe('light');
  });

  it('reports system changes', () => {
    const listeners: ((e: { matches: boolean }) => void)[] = [];
    const query = {
      matches: false,
      addEventListener: (_: string, l: (e: { matches: boolean }) => void) => listeners.push(l),
      removeEventListener: () => listeners.pop(),
    };
    const seen: boolean[] = [];
    const stop = watchSystemDark(() => query, (dark) => seen.push(dark));
    listeners[0]?.({ matches: true });
    stop();
    expect(seen).toEqual([false, true]);
    expect(listeners).toHaveLength(0);
  });

  it('steps system, light, dark and round, as ui.theme.cycle does', () => {
    expect(nextTheme('system')).toBe('light');
    expect(nextTheme('light')).toBe('dark');
    expect(nextTheme('dark')).toBe('system');
  });
});
