import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { LOCALE_FILE, TABLE_FILE, tableText } from './gen_rules.ts';
import { CATEGORIES } from './params.ts';
import { auditTable, installedConfig } from './rules_table.ts';

describe('auditTable', () => {
  it('lists each audit once with its categories and highest weight', () => {
    const config = {
      categories: {
        performance: { auditRefs: [{ id: 'lcp', weight: 25 }, { id: 'diag', weight: 0 }] },
        accessibility: { auditRefs: [{ id: 'image-alt', weight: 10 }] },
        seo: { auditRefs: [{ id: 'diag', weight: 1 }] },
        pwa: { auditRefs: [{ id: 'ignored', weight: 1 }] },
      },
      titles: new Map([
        ['lcp', 'Largest Contentful Paint'],
        ['diag', 'Diagnostics'],
        ['image-alt', 'Image elements have `[alt]` attributes'],
      ]),
    };
    const { table, titles } = auditTable('13.6.0', config, ['performance', 'accessibility', 'seo']);
    expect(table).toEqual({
      lighthouseVersion: '13.6.0',
      audits: [
        { id: 'diag', categories: ['performance', 'seo'], weight: 1 },
        { id: 'image-alt', categories: ['accessibility'], weight: 10 },
        { id: 'lcp', categories: ['performance'], weight: 25 },
      ],
    });
    expect(titles.lcp).toBe('Largest Contentful Paint');
  });

  it('refuses an audit without a title', () => {
    const config = { categories: { seo: { auditRefs: [{ id: 'x', weight: 1 }] } }, titles: new Map<string, string>() };
    expect(() => auditTable('1', config, ['seo'])).toThrow('audit x has no title');
  });
});

describe("the plugin's committed table", () => {
  it('is the pinned Lighthouse\'s (node src/modules/lighthouse/gen_rules.ts writes it)', { timeout: 30_000 }, async () => {
    const { version, config } = await installedConfig();
    const { table, titles } = auditTable(version, config, CATEGORIES);
    expect(readFileSync(TABLE_FILE, 'utf8')).toBe(tableText(table));
    const locale = JSON.parse(readFileSync(LOCALE_FILE, 'utf8')) as { plugin: { lighthouse: { rules: unknown } } };
    expect(locale.plugin.lighthouse.rules).toEqual(titles);
  });
});
