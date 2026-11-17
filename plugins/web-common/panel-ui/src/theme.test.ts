import { describe, expect, it } from 'vitest';
import { applyTextScale, applyTheme, textScaleOf, themeFollower, themeOf } from './theme';

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

describe('the panel text size (RB-090-39)', () => {
  it('reads ui.textScale held to 100..225, 100 for anything else', () => {
    expect(textScaleOf({ 'ui.textScale': 150 })).toBe(150);
    expect(textScaleOf({ 'ui.textScale': 300 })).toBe(225);
    expect(textScaleOf({ 'ui.textScale': 50 })).toBe(100);
    expect(textScaleOf({ 'ui.textScale': 137.6 })).toBe(138);
    expect(textScaleOf({ 'ui.textScale': '200' })).toBe(100);
    expect(textScaleOf({ 'ui.textScale': Number.NaN })).toBe(100);
    expect(textScaleOf(undefined)).toBe(100);
  });

  it("sets the root font size, the page's own at 100 %", () => {
    const root = document.createElement('html');
    applyTextScale(root, 200);
    expect(root.style.fontSize).toBe('200%');
    applyTextScale(root, 100);
    expect(root.style.fontSize).toBe('');
  });

  it('follows ui.textScale with every settings message', () => {
    const root = document.createElement('html');
    const apply = themeFollower(root, undefined);
    expect(root.style.fontSize).toBe('');
    apply({ 'ui.theme': 'dark', 'ui.textScale': 150 });
    expect(root.style.fontSize).toBe('150%');
    apply({ 'ui.theme': 'dark', 'ui.textScale': 225 });
    expect(root.style.fontSize).toBe('225%');
    apply({ 'ui.theme': 'dark' });
    expect(root.style.fontSize).toBe('');
  });
});
