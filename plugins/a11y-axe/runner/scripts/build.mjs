// Builds what the plugin's component embeds (src/script.rs, src/rules.rs) into dist/, which is
// checked in so the Rust build never needs npm:
//
// - dist/axe.min.js: axe-core's own file, the exact version package.json pins (MPL-2.0, see
//   ../NOTICE), byte for byte, with its licence texts beside it (dist/axe-core.LICENSE*);
// - dist/runner.js: src/main.ts as one minified IIFE (`var rbA11yAxe = ...`);
// - dist/rules.json: axe's rules - id, usual impact, tags, help text and help page;
// - ../locales/en.json: the rule titles `plugin.a11y-axe.rules.<id>` (axe's English help text),
//   the other keys untouched.
//
// src/dist.test.ts holds the four to node_modules/axe-core.

import { build } from 'esbuild';
import { copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const BUDGET = 16 * 1024;

await build({
  entryPoints: ['src/main.ts'],
  outfile: 'dist/runner.js',
  bundle: true,
  format: 'iife',
  globalName: 'rbA11yAxe',
  minify: true,
  target: 'chrome120',
  legalComments: 'none',
  logLevel: 'warning',
});
const runner = readFileSync('dist/runner.js', 'utf8');
if (runner.length > BUDGET) {
  console.error(`dist/runner.js is ${runner.length} bytes, over the budget of ${BUDGET}`);
  process.exit(1);
}

copyFileSync(require.resolve('axe-core/axe.min.js'), 'dist/axe.min.js');
copyFileSync(require.resolve('axe-core/LICENSE'), 'dist/axe-core.LICENSE');
copyFileSync(require.resolve('axe-core/LICENSE-3RD-PARTY.txt'), 'dist/axe-core.LICENSE-3RD-PARTY.txt');

const axe = require('axe-core');
const rules = axe._audit.rules
  .map((rule) => {
    const meta = axe.getRules().find((entry) => entry.ruleId === rule.id);
    return {
      id: rule.id,
      impact: rule.impact ?? null,
      tags: [...rule.tags],
      help: meta?.help ?? '',
      helpUrl: meta?.helpUrl ?? '',
    };
  })
  .sort((a, b) => a.id.localeCompare(b.id));
writeFileSync('dist/rules.json', `${JSON.stringify({ axeVersion: axe.version, rules }, null, 2)}\n`);

function sorted(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, sorted(value[key])]),
  );
}

const localePath = '../locales/en.json';
const locale = JSON.parse(readFileSync(localePath, 'utf8'));
locale.plugin['a11y-axe'].rules = Object.fromEntries(rules.map((rule) => [rule.id, rule.help]));
writeFileSync(localePath, `${JSON.stringify(sorted(locale), null, 2)}\n`);

console.log(`dist/: axe-core ${axe.version}, ${rules.length} rules, runner ${runner.length} bytes`);
