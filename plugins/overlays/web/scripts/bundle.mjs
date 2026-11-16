// Bundles the plugin's two page-side scripts, each one minified IIFE (classic script, no module):
// - src/page.ts      -> ../page/overlays.js   the page script (manifest [[page_scripts]])
// - src/measure-entry.ts -> ../assets/measure.js  the measurement `overlay_overflow` evaluates
//   (src/report.rs includes it; it defines the global `__rbOverlaysMeasure`)
// rb-plugin-pack packs every file below page/ and panel/, so nothing else is written there.
// Fails above a size budget of 40 kB for either. The panel is built by Vite (vite.config.ts,
// RB-080-86).

import { build } from 'esbuild';
import { statSync } from 'node:fs';

const BUDGET = 40 * 1024;
const BUNDLES = [
  { entry: 'src/page.ts', out: '../page/overlays.js' },
  { entry: 'src/measure-entry.ts', out: '../assets/measure.js', globalName: '__rbOverlaysMeasure' },
];

let failed = false;
for (const { entry, out, globalName } of BUNDLES) {
  await build({
    entryPoints: [entry],
    outfile: out,
    bundle: true,
    format: 'iife',
    globalName,
    minify: true,
    target: 'chrome120',
    legalComments: 'none',
    logLevel: 'warning',
    banner: { js: '// Built from plugins/overlays/web (npm run build); do not edit.' },
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
