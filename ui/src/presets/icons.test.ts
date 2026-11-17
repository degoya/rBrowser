import { describe, expect, it } from 'vitest';
import { BUNDLED_ICONS } from '../icons';
import { PRESET_ICONS } from './icons';

// Icons drawn as four corners: zoom to fit's look (owner, build 22481e2).
const CORNER_ICONS = ['i-lucide-scan', 'i-lucide-maximize', 'i-lucide-fullscreen', 'i-lucide-focus'];

describe('the layout icons', () => {
  it('give every control an icon of its own', () => {
    const values = Object.values(PRESET_ICONS);
    expect(new Set(values).size).toBe(values.length);
  });

  it('do not draw focus mode like zoom to fit', () => {
    expect(PRESET_ICONS.focusMode).not.toBe(PRESET_ICONS.zoomToFit);
    expect(CORNER_ICONS).not.toContain(PRESET_ICONS.focusMode);
  });

  it('are bundled', () => {
    expect(BUNDLED_ICONS).toContain(PRESET_ICONS.focusMode.replace(/^i-lucide-/, 'lucide:'));
  });
});
