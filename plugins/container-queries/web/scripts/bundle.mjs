// Bundles the plugin's two page-side scripts, each one minified IIFE (classic script, no module):
// - src/page.ts          -> ../page/containers.js  the page script (manifest [[page_scripts]])
// - src/analyze-entry.ts -> ../assets/analyze.js   what the component evaluates (src/script.rs
//   includes it; it defines the global `__rbContainers`)
// Committed - scripts/build-plugins.sh packs and builds without npm - and src/bundle.test.ts fails
// when one is stale. rb-plugin-pack packs every file below page/ and panel/, so nothing else is
// written there. Fails above a size budget of 40 kB for either. The panel is built by Vite
// (vite.config.ts).

import { build } from 'esbuild';
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const BUDGET = 40 * 1024;

export const BUNDLES = [
  { entry: 'src/page.ts', out: '../page/containers.js' },
  { entry: 'src/analyze-entry.ts', out: '../assets/analyze.js', globalName: '__rbContainers' },
];

const HEADER = '// Built from plugins/container-queries/web (npm run build); do not edit.\n';

/** The text of one bundle, as `npm run build` writes it. */
export async function bundle({ entry, globalName }) {
  const result = await build({
    absWorkingDir: resolve(here, '..'),
    entryPoints: [entry],
    bundle: true,
    write: false,
    format: 'iife',
    globalName,
    minify: true,
    charset: 'utf8',
    target: 'chrome120',
    legalComments: 'none',
    logLevel: 'warning',
  });
  return HEADER + result.outputFiles[0].text;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let failed = false;
  for (const spec of BUNDLES) {
    const text = await bundle(spec);
    writeFileSync(resolve(here, '..', spec.out), text);
    const size = Buffer.byteLength(text);
    if (size > BUDGET) {
      console.error(`${spec.out} is ${size} bytes, over the budget of ${BUDGET}`);
      failed = true;
    } else {
      console.log(`${spec.out}: ${size} bytes (budget ${BUDGET})`);
    }
  }
  if (failed) process.exit(1);
}
