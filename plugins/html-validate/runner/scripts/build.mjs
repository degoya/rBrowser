// Builds what the plugin's component embeds (src/script.rs, src/rules.rs) into dist/, which is
// checked in so the Rust build never needs npm:
//
// - dist/runner.js: src/main.ts with html-validate's browser build as one minified IIFE
//   (`var rbHtmlValidate = ...`); the licences of what it bundles beside it
//   (dist/runner.LICENSES.txt, see ../NOTICE);
// - dist/rules.json: the rules a run offers - html-validate's `html-validate:standard` preset
//   (standards conformance, no style rules) without its two rules about html-validate's own
//   comment directives, plus `missing-doctype` and `no-missing-references` from its `document`
//   preset, each with its configuration (severity and options) and help page; the opt-in rules
//   (OPT_IN below), off until a project's `.rbrowser/htmlvalidate.json` switches them on; and the
//   three rules of the Nu HTML Checker's message types;
// - ../locales/en.json: the rule titles `plugin.html-validate.rules.<id>` (TITLES below), the
//   other keys untouched.
//
// src/dist.test.ts holds the three to node_modules/html-validate.

import { build } from 'esbuild';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const BUDGET = 1536 * 1024;

const result = await build({
  entryPoints: ['src/main.ts'],
  outfile: 'dist/runner.js',
  bundle: true,
  format: 'iife',
  globalName: 'rbHtmlValidate',
  platform: 'browser',
  minify: true,
  target: 'chrome120',
  legalComments: 'none',
  metafile: true,
  logLevel: 'warning',
});
const runner = readFileSync('dist/runner.js', 'utf8');
if (runner.length > BUDGET) {
  console.error(`dist/runner.js is ${runner.length} bytes, over the budget of ${BUDGET}`);
  process.exit(1);
}

// The npm packages the bundle holds, each with its licence text.
const packages = new Map();
for (const input of Object.keys(result.metafile.inputs)) {
  const match = input.match(/^(.*node_modules\/((?:@[^/]+\/)?[^/]+))\//);
  if (match) packages.set(match[2], match[1]);
}
const licences = [...packages.keys()].sort().map((name) => {
  const root = packages.get(name);
  const meta = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  const file = ['LICENSE', 'LICENSE.md', 'license', 'LICENSE.txt']
    .map((candidate) => join(root, candidate))
    .find((candidate) => {
      try {
        readFileSync(candidate);
        return true;
      } catch {
        return false;
      }
    });
  const text = file ? readFileSync(file, 'utf8').trim() : `Licence: ${meta.license}`;
  return `${meta.name} ${meta.version} (${meta.license})\n\n${text}\n`;
});
writeFileSync('dist/runner.LICENSES.txt', `${licences.join('\n----------------------------------------\n\n')}`);

const presets = await import('html-validate/presets');
const { version } = await import('html-validate');

/** Rules of `html-validate:standard` about html-validate's own directives, not about HTML. */
const LEFT_OUT = ['deprecated-rule', 'no-unused-disable'];
/** Rules of `html-validate:document` that are conformance (the W3C validator reports both). */
const FROM_DOCUMENT = ['missing-doctype', 'no-missing-references'];

/** Rules in no preset the plugin takes, off by default: `no-unknown-attributes` reports what
 * the W3C validator calls "attribute not allowed", but html-validate 11.18.0's element table
 * misses valid attributes (`loading` and `allow` on `<iframe>`, `viewBox` on `<svg>`), so only a
 * project switches it on, for pages it knows (RB-094-06). */
const OPT_IN = ['no-unknown-attributes'];

/** The English titles of the rules (the locale's `rules` group). */
const TITLES = {
  'area-alt': 'Image map areas have alternative text',
  'aria-label-misuse': 'aria-label only where the element can be named',
  'attr-spacing': 'Attributes are separated by whitespace',
  'attribute-allowed-values': 'Attribute values are allowed for the element',
  'attribute-misuse': 'Attributes are used where they apply',
  'close-attr': 'End tags have no attributes',
  'close-order': 'Elements are closed in the right order',
  deprecated: 'No obsolete elements',
  'doctype-html': 'The doctype is the HTML doctype',
  'element-name': 'Custom element names are valid',
  'element-permitted-content': 'Elements hold only the content they permit',
  'element-permitted-occurrences': 'Elements occur no more often than permitted',
  'element-permitted-order': 'Child elements come in the required order',
  'element-permitted-parent': 'Elements sit in a parent that permits them',
  'element-required-ancestor': 'Elements have their required ancestor',
  'element-required-attributes': 'Elements have their required attributes',
  'element-required-content': 'Elements have their required children',
  'map-dup-name': 'Image map names are unique',
  'map-id-name': 'Image map id and name match',
  'missing-doctype': 'The document has a doctype',
  'multiple-labeled-controls': 'A label names one control only',
  'no-abstract-role': 'No abstract ARIA roles',
  'no-deprecated-attr': 'No obsolete attributes',
  'no-dup-attr': 'No attribute is repeated on an element',
  'no-dup-id': 'Element ids are unique',
  'no-missing-references': 'References point to ids that exist',
  'no-multiple-main': 'At most one visible main element',
  'no-raw-characters': 'Special characters are escaped',
  'no-unknown-attributes': 'Attributes are known for their element',
  'script-element': 'Script elements have an end tag',
  'unrecognized-char-ref': 'Character references are known ones',
  'valid-autocomplete': 'autocomplete values are valid',
  'valid-for': 'label for attributes are valid ids',
  'valid-id': 'Element ids are valid',
  'void-content': 'Void elements have no content',
  'nu-error': 'Nu HTML Checker: error',
  'nu-warning': 'Nu HTML Checker: warning',
  'nu-info': 'Nu HTML Checker: information',
};

/** The Nu HTML Checker's message types as rules (src/nu.ts). */
const NU_RULES = [
  { id: 'nu-error', severity: 'error', config: null, url: '' },
  { id: 'nu-warning', severity: 'warning', config: null, url: '' },
  { id: 'nu-info', severity: 'info', config: null, url: '' },
];

const standard = Object.entries(presets.standard.rules).filter(([id]) => !LEFT_OUT.includes(id));
const document = FROM_DOCUMENT.map((id) => [id, presets.document.rules[id]]);
const optIn = OPT_IN.map((id) => [id, 'off']);
const builtin = [...standard, ...document, ...optIn]
  .map(([id, config]) => {
    const level = Array.isArray(config) ? config[0] : config;
    return {
      id,
      // An opt-in rule's findings are errors once a project switches it on with `error`.
      severity: level === 'warn' ? 'warning' : 'error',
      config,
      url: `https://html-validate.org/rules/${id}.html`,
    };
  })
  .sort((a, b) => a.id.localeCompare(b.id));
const rules = [...builtin, ...NU_RULES];
const untitled = rules.filter((rule) => !TITLES[rule.id]).map((rule) => rule.id);
if (untitled.length > 0) {
  console.error(`rules without a title in scripts/build.mjs: ${untitled.join(', ')}`);
  process.exit(1);
}
writeFileSync(
  'dist/rules.json',
  `${JSON.stringify({ htmlValidateVersion: version, preset: 'html-validate:standard', rules }, null, 2)}\n`,
);

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
locale.plugin['html-validate'].rules = Object.fromEntries(rules.map((rule) => [rule.id, TITLES[rule.id]]));
writeFileSync(localePath, `${JSON.stringify(sorted(locale), null, 2)}\n`);

console.log(
  `dist/: html-validate ${version}, ${rules.length} rules, runner ${runner.length} bytes, ${packages.size} packages bundled`,
);
