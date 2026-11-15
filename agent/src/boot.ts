// The boot of the agent in a frame's isolated world (RB-020-01). The core injects the bundle with
// `Page.addScriptToEvaluateOnNewDocument { worldName: "rbrowser", runImmediately: true }` after
// `Runtime.addBinding { executionContextName: "rbrowser" }`, so the binding and every global
// here live in that world only: the page's main world sees none of them, and the page's CSP
// does not apply. The boot configuration comes in front of the bundle as `__rbAgentBoot`.

import { Agent, isConfig } from './agent';
import { AGENT_GLOBAL, BINDING_NAME, BOOT_GLOBAL, DEFAULT_CONFIG } from './protocol';

/** Starts the agent in `scope` (the world's global object) for `win`; false when it was
 *  already there or the binding is missing. */
export function boot(scope: Record<string, unknown>, win: Window): boolean {
  if (scope[AGENT_GLOBAL]) return false;
  const binding = scope[BINDING_NAME];
  if (typeof binding !== 'function') return false;
  const config = scope[BOOT_GLOBAL];
  delete scope[BOOT_GLOBAL];
  const agent = new Agent({
    win,
    send: (payload) => {
      (binding as (payload: string) => void)(payload);
    },
    config: isConfig(config) ? config : DEFAULT_CONFIG,
  });
  Object.defineProperty(scope, AGENT_GLOBAL, {
    value: Object.freeze({ command: (input: unknown) => agent.command(input) }),
    enumerable: false,
    writable: false,
    configurable: false,
  });
  agent.start();
  return true;
}
