import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { BUNDLED_ICONS } from '../icons';
import { VIDEO_ICONS } from './icons';

describe('the Videos panel icons', () => {
  it('are Lucide icons, one per control, and bundled', () => {
    const lucide = JSON.parse(
      readFileSync(join(process.cwd(), 'node_modules/@iconify-json/lucide/icons.json'), 'utf-8'),
    ) as { icons: Record<string, unknown> };
    const values = Object.values(VIDEO_ICONS);
    expect(new Set(values).size).toBe(values.length);
    for (const icon of values) {
      expect(lucide.icons[icon.replace(/^i-lucide-/, '')], icon).toBeDefined();
      expect(BUNDLED_ICONS).toContain(icon.replace(/^i-lucide-/, 'lucide:'));
    }
  });
});
