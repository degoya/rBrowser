// Bundles the plugin's page script, one minified IIFE (classic script, no module):
// - src/page.ts       -> ../page/simulators.js  the page script (manifest [[page_scripts]])
// rb-plugin-pack packs every file below page/ and panel/, so nothing else is written there.
// Fails above a size budget of 24 kB. The picker is built by Vite (vite.config.ts, RB-080-86).

import { build } from 'esbuild';
import { statSync } from 'node:fs';

const BUDGET = 24 * 1024;
const BUNDLES = [
  { entry: 'src/page.ts', out: '../page/simulators.js' },
];

let failed = false;
for (const { entry, out } of BUNDLES) {
  await build({
    entryPoints: [entry],
    outfile: out,
    bundle: true,
    format: 'iife',
    minify: true,
    target: 'chrome120',
    legalComments: 'none',
    logLevel: 'warning',
    banner: { js: '// Built from plugins/simulators/web (npm run build); do not edit.' },
  });
  const size = statSync(out).size;
  if (size > BUDGET) {
    console.error(`${out} is ${size} bytes, over the budget of ${BUDGET}`);
    failed = true;
  } else {
    console.log(`${out}: ${size} bytes (budget ${BUDGET})`);
  }
}
if (failed) process.exit(1);
