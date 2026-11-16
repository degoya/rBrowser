// Writes the `lighthouse` plugin's audit table from the pinned Lighthouse (rules_table.ts):
// plugins/lighthouse/data/rules.json and the `rules` titles of plugins/lighthouse/locales/en.json.
// Run after a Lighthouse update: `node src/modules/lighthouse/gen_rules.ts` in sidecar/.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { CATEGORIES } from './params.ts';
import { auditTable, installedConfig } from './rules_table.ts';

/** The plugin's folder, seen from this file. */
export const PLUGIN_DIR = fileURLToPath(new URL('../../../../plugins/lighthouse/', import.meta.url));
export const TABLE_FILE = `${PLUGIN_DIR}data/rules.json`;
export const LOCALE_FILE = `${PLUGIN_DIR}locales/en.json`;

/** The table as committed: two-space JSON with a final newline. */
export function tableText(table: unknown): string {
  return `${JSON.stringify(table, null, 2)}\n`;
}

async function main(): Promise<void> {
  const { version, config } = await installedConfig();
  const { table, titles } = auditTable(version, config, CATEGORIES);
  writeFileSync(TABLE_FILE, tableText(table));
  const locale = JSON.parse(readFileSync(LOCALE_FILE, 'utf8')) as { plugin: { lighthouse: Record<string, unknown> } };
  locale.plugin.lighthouse.rules = titles;
  writeFileSync(LOCALE_FILE, `${JSON.stringify(locale, null, 2)}\n`);
  console.error(`${table.audits.length} audits of Lighthouse ${version} written`);
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  await main();
}
