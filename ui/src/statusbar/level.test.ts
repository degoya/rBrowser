import { describe, expect, it } from 'vitest';
import { LEVELS, levelOf, fades, formatTime, messageLine } from './level';

describe('the message levels', () => {
  it('name each level with an icon, a word and a colour; errors are assertive', () => {
    expect(LEVELS).toEqual(['error', 'warning', 'info', 'success']);
    for (const level of LEVELS) {
      const look = levelOf(level);
      expect(look.icon).toMatch(/^i-lucide-/);
      expect(look.wordKey).toBe(`status.level.${level}`);
      expect(look.color).toMatch(/^text-/);
    }
    expect(levelOf('error').live).toBe('assertive');
    expect(levelOf('warning').live).toBe('polite');
    expect(levelOf('info').live).toBe('polite');
  });

  it('lets info and success fade, never a warning or an error', () => {
    expect(fades('info')).toBe(true);
    expect(fades('success')).toBe(true);
    expect(fades('warning')).toBe(false);
    expect(fades('error')).toBe(false);
  });

  it('writes the time as hours, minutes and seconds', () => {
    const at = new Date(2026, 10, 12, 9, 5, 7).getTime();
    expect(formatTime(at)).toMatch(/09.05.07/);
  });

  it('writes a copied line as time, level word and text', () => {
    const at = new Date(2026, 10, 12, 9, 5, 7).getTime();
    expect(messageLine(at, 'Error', 'It failed.')).toMatch(/^09.05.07\tError\tIt failed\.$/);
  });
});
