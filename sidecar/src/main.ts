// The sidecar's entry point: JSON-RPC 2.0 over stdio, one message per line (RB-040-03). stdout
// carries the protocol only, so every console output goes to stderr, which rb-sidecar writes
// into rBrowser's log. No network listener: stdio is the only channel. The sidecar ends when
// rBrowser closes its stdin.

import { LineSplitter, MAX_LINE_BYTES } from './framing.ts';
import { realFlowsDeps } from './modules/flows/real.ts';
import { registerFlows } from './modules/flows/spawn.ts';
import { EXIT_AFTER_MS } from './modules/lighthouse/budget.ts';
import { lighthouseDangling } from './modules/lighthouse/dangling.ts';
import { realLighthouseDeps, registerLighthouse } from './modules/lighthouse/index.ts';
import { browserStates, registerPlaywright } from './modules/playwright.ts';
import { registerSidecar } from './modules/sidecar.ts';
import { playwrightVersion, realPlaywrightDeps } from './playwright_core.ts';
import { ErrorCode } from './rpc.ts';
import { Router } from './router.ts';
import { handleRejections } from './rejections.ts';
import { RpcServer } from './server.ts';
import { registerWebkit } from './webkit/index.ts';
import { realServiceDeps } from './webkit/real.ts';
import { WebkitService } from './webkit/service.ts';

const toStderr = (...args: unknown[]) => console.error(...args);
console.log = toStderr;
console.info = toStderr;
console.debug = toStderr;
handleRejections(process, {
  log: (line) => console.error(line),
  exit: (code) => process.exit(code),
  tolerates: (reason) => lighthouseDangling.tolerates(reason),
});

const playwright = realPlaywrightDeps(process.env);
const router = new Router();
registerSidecar(router, { playwrightVersion, browsers: () => browserStates(playwright) });
registerPlaywright(router, playwright);
registerFlows(router, realFlowsDeps(import.meta.url));
const lighthouse = registerLighthouse(router, realLighthouseDeps());

const server = new RpcServer(router, (line) => {
  process.stdout.write(line);
});
const webkit = new WebkitService(
  realServiceDeps(playwright.browsersPath, playwright.entries, (method, params) => server.notify(method, params)),
);
registerWebkit(router, webkit);
const splitter = new LineSplitter(
  MAX_LINE_BYTES,
  (line) => {
    void server.handleLine(line);
  },
  (bytes) => server.writeError(null, { code: ErrorCode.ParseError, message: `line of ${bytes} bytes too long` }),
);

process.stdin.on('data', (chunk: Buffer) => splitter.push(chunk));
// WebKit's windows and the running Lighthouse browser close with the sidecar (every caller's
// run, RB-080-67); the sidecar exits after `EXIT_AFTER_MS` at the latest, inside rb-sidecar's
// STOP_GRACE (budget.ts), which kills a sidecar that takes longer - its job object the rest.
process.stdin.on('end', () => {
  setTimeout(() => process.exit(0), EXIT_AFTER_MS).unref();
  void Promise.allSettled([lighthouse.shutdown(), webkit.shutdown()]).finally(() => process.exit(0));
});
