// The panels' icons (RB-080-61): every bundled plugin's panel names an icon the shell bundles,
// each a real Lucide icon, and nothing but a bundled name ever reaches `UIcon` - an unknown name,
// a URL or markup shows the fallback, so the page never asks the network for an icon.

import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { BUNDLED_ICONS } from '../icons';
import { FALLBACK_PANEL_ICON, PANEL_BUNDLED_ICONS, PANEL_ICON_NAMES, panelIcon } from './icons';

// The repository root: the tests run in ui/ (`npm test`).
const ROOT = join(process.cwd(), '..');

/** `[plugin, panel, icon?]` of every `[[panels]]` entry of the bundled plugins' manifests. */
function bundledPanels(): [string, string, string | undefined][] {
  const rows: [string, string, string | undefined][] = [];
  const plugins = join(ROOT, 'plugins');
  for (const plugin of readdirSync(plugins)) {
    const path = join(plugins, plugin, 'manifest.toml');
    if (!existsSync(path)) {
      continue;
    }
    for (const block of readFileSync(path, 'utf-8').split(/^\[\[panels\]\]$/m).slice(1)) {
      const table = block.split(/^\[/m)[0] ?? '';
      const id = /^id = "([^"]+)"/m.exec(table)?.[1] ?? '?';
      rows.push([plugin, id, /^icon = "([^"]+)"/m.exec(table)?.[1]]);
    }
  }
  return rows;
}

describe('the panels icons', () => {
  it('every bundled panel names an icon the shell bundles', () => {
    const rows = bundledPanels();
    expect(rows.length).toBeGreaterThan(15);
    for (const [plugin, panel, icon] of rows) {
      expect(icon, `${plugin}.${panel} names no icon`).toBeDefined();
      expect(PANEL_ICON_NAMES, `${plugin}.${panel}: ${icon}`).toContain(icon);
    }
  });

  it('bundles every name, each a Lucide icon, and the fallback', () => {
    const lucide = JSON.parse(
      readFileSync(join(process.cwd(), 'node_modules/@iconify-json/lucide/icons.json'), 'utf-8'),
    ) as { icons: Record<string, unknown> };
    for (const name of [...PANEL_ICON_NAMES, FALLBACK_PANEL_ICON]) {
      expect(lucide.icons[name], name).toBeDefined();
    }
    for (const name of PANEL_BUNDLED_ICONS) {
      expect(BUNDLED_ICONS).toContain(name);
    }
    expect(BUNDLED_ICONS).toContain(`lucide:${FALLBACK_PANEL_ICON}`);
  });

  it('draws a bundled name and the fallback for anything else', () => {
    expect(panelIcon('type')).toBe('i-lucide-type');
    for (const other of [
      undefined,
      null,
      '',
      'not-bundled-icon',
      'https://example.com/x.svg',
      '<svg onload=alert(1)>',
      'i-lucide-type',
      'type extra',
    ]) {
      expect(panelIcon(other), String(other)).toBe(`i-lucide-${FALLBACK_PANEL_ICON}`);
    }
  });
});
