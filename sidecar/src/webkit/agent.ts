// The sync agent in a WebKit view (RB-040-04). Playwright has no public isolated world for
// WebKit, so the agent runs in the page's main world: `context.addInitScript` runs the script
// below before the page's own scripts in every frame, and `context.exposeBinding` is its way
// back. The bundle (agent/dist/rb-agent.js, sent by rBrowser) runs inside a function whose
// parameter `globalThis` is a private scope object, so the boot configuration, the agent's state
// and its `__rbEmit`/`__rbAgent` names never reach the page's globals.
//
// What the page can still see - the deviation from Chromium's isolated world, recorded in
// ADR 0001 and docs/security/threat-model.md: the binding (a random name per launch, enumerable
// as Playwright installs it) and the agent's command entry (a random, non-enumerable name). A
// page that looks for them can send forged reports - rBrowser parses every report as untrusted
// input (rb_sync::parse_message), and they count only while the WebKit view is the active view
// with `engines.webkit.syncSource` on - and can ask the agent to resolve selectors in its own
// document, which it could do itself. The page's CSP does not block the script: Playwright
// injects it outside the page's script loading.

/** The agent's globals inside the scope (`agent/src/protocol.ts`). */
const BINDING_NAME = '__rbEmit';
const BOOT_GLOBAL = '__rbAgentBoot';
const AGENT_GLOBAL = '__rbAgent';

/** The largest report forwarded (`rb_sync::MAX_MESSAGE_BYTES`); longer ones are dropped. */
export const MAX_REPORT_BYTES = 256 * 1024;

/** The random names of one browser: the binding and the agent's command entry. */
export interface AgentNames {
  binding: string;
  agent: string;
}

/** Names nobody guesses: `__rb` and 16 random bytes in hex. */
export function randomNames(random: (bytes: number) => Uint8Array): AgentNames {
  const name = () => `__rb${Buffer.from(random(16)).toString('hex')}`;
  return { binding: name(), agent: name() };
}

/** The init script: the agent `bundle` booted with `config` in a private scope. */
export function agentScript(bundle: string, config: unknown, names: AgentNames): string {
  const binding = JSON.stringify(names.binding);
  const agent = JSON.stringify(names.agent);
  return [
    '(() => {',
    '"use strict";',
    `const emit = globalThis[${binding}];`,
    'if (typeof emit !== "function") return;',
    'const scope = {',
    `  ${BINDING_NAME}: (payload) => { emit(String(payload)); },`,
    `  ${BOOT_GLOBAL}: ${JSON.stringify(config ?? null)},`,
    '};',
    '(function (globalThis) {',
    bundle,
    '}).call(undefined, scope);',
    `const handle = scope.${AGENT_GLOBAL};`,
    'if (handle !== undefined) {',
    `  Object.defineProperty(globalThis, ${agent}, { value: handle, enumerable: false, configurable: false, writable: false });`,
    '}',
    '})();',
  ].join('\n');
}

/** Runs `command` in the agent of the page's main frame (evaluated in the page). */
export function runAgentCommand([name, command]: readonly [string, unknown]): unknown {
  const scope = globalThis as unknown as Record<string, { command?: (input: unknown) => unknown } | undefined>;
  const handle = scope[name];
  if (handle === undefined || typeof handle.command !== 'function') {
    return { status: 'error', message: 'no agent in this page' };
  }
  return handle.command(command);
}

/** A report as it is forwarded: a string within the size limit, else `null`. */
export function reportPayload(payload: unknown): string | null {
  if (typeof payload !== 'string' || Buffer.byteLength(payload, 'utf8') > MAX_REPORT_BYTES) {
    return null;
  }
  return payload;
}
