import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { BUNDLED_ICONS, ICONS } from '../icons';
import { RECORDER_ICONS } from './icons';

describe('the Recorder and Flows panels icons (RB-090-60)', () => {
  it('are Lucide icons, bundled, and override no other key of ICONS', () => {
    const lucide = JSON.parse(
      readFileSync(join(process.cwd(), 'node_modules/@iconify-json/lucide/icons.json'), 'utf-8'),
    ) as { icons: Record<string, unknown> };
    for (const [key, icon] of Object.entries(RECORDER_ICONS)) {
      expect(lucide.icons[icon.replace(/^i-lucide-/, '')], icon).toBeDefined();
      expect(BUNDLED_ICONS).toContain(icon.replace(/^i-lucide-/, 'lucide:'));
      expect(ICONS[key as keyof typeof ICONS], key).toBe(icon);
    }
  });
});
