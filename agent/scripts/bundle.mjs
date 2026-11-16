// Bundles the scripts rbrowser's build script embeds (crates/rbrowser/build.rs), each one
// minified IIFE: src/main.ts, the sync agent of the isolated world, into dist/rb-agent.js,
// src/page-api/main.ts, the page API of the main world (RB-020-08), into dist/rb-page-api.js,
// src/portal/main.ts, the agent of the portal's devices (RB-080-05), into
// dist/rb-portal-agent.js, and the main-world scripts of the Framework DevTools (RB-080-24):
// src/framework/main-probe.ts, the framework probe, into dist/rb-framework-probe.js,
// src/framework/main-react.ts, the React DevTools backend (react-devtools-inline with react),
// into dist/rb-react-devtools.js and src/framework/main-vue.ts, the Vue DevTools backend
// (@vue/devtools-kit and @vue/devtools-core), into dist/rb-vue-devtools.js.
// Fails above the size budget of RB-020-01 (50 kB) for any of them; the two DevTools backends
// carry third-party code and have budgets of their own (the spike's sizes plus a margin).

import { build } from 'esbuild';
import { statSync } from 'node:fs';

const BUDGET = 50 * 1024;
const BUNDLES = [
  { entry: 'src/main.ts', out: 'dist/rb-agent.js' },
  { entry: 'src/page-api/main.ts', out: 'dist/rb-page-api.js' },
  // The portal agent (RB-080-05) runs on real devices: Safari on iOS as well as Chrome.
  { entry: 'src/portal/main.ts', out: 'dist/rb-portal-agent.js', target: ['chrome120', 'safari15'] },
  { entry: 'src/framework/main-probe.ts', out: 'dist/rb-framework-probe.js' },
  { entry: 'src/framework/main-react.ts', out: 'dist/rb-react-devtools.js', budget: 320 * 1024 },
  { entry: 'src/framework/main-vue.ts', out: 'dist/rb-vue-devtools.js', budget: 160 * 1024 },
];

let failed = false;
for (const { entry, out, target = 'chrome120', budget = BUDGET } of BUNDLES) {
  await build({
    entryPoints: [entry],
    outfile: out,
    bundle: true,
    format: 'iife',
    minify: true,
    target,
    legalComments: 'none',
    logLevel: 'warning',
    // The third-party backends read these as a bundler's constants.
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  const size = statSync(out).size;
  if (size > budget) {
    console.error(`${out} is ${size} bytes, over the budget of ${budget}`);
    failed = true;
  } else {
    console.log(`${out}: ${size} bytes (budget ${budget})`);
  }
}
if (failed) process.exit(1);
