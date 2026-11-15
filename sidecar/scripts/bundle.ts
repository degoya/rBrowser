// `npm run build`: bundles src/main.ts into dist/main.js, the file rb-sidecar starts, and the flow
// worker into dist/flow-worker.js beside it (RB-040-07). Runs as TypeScript on Node directly.
// playwright-core and @playwright/test stay outside the bundles (they read their own files and
// start their own CLI; RB-040-04 downloads its WebKit build itself); the package ships them in
// sidecar/node_modules.

import { type BuildOptions, build } from 'esbuild';

const common: BuildOptions = {
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node26',
  external: ['playwright-core', '@playwright/test'],
  legalComments: 'none',
  logLevel: 'warning',
};

await build({ ...common, entryPoints: ['src/main.ts'], outfile: 'dist/main.js' });
await build({ ...common, entryPoints: ['src/modules/flows/worker.ts'], outfile: 'dist/flow-worker.js' });

