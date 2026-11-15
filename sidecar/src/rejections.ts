// A rejected promise nobody waits for ends Node - and that stays so (review 1 of RB-080-67): a
// bug in WebKit, flows or the RPC must not leave a broken sidecar running; rb-sidecar restarts
// it. The one exception is the rejection Lighthouse leaves behind when a cancelled run ends its
// browser under it (`lighthouse/dangling.ts`): logged, not fatal.

/** The part of `process` this needs. */
export interface RejectionSource {
  on(event: 'unhandledRejection', listener: (reason: unknown) => void): unknown;
}

export interface RejectionDeps {
  log(line: string): void;
  exit(code: number): void;
  /** Whether a rejection is a known, harmless one. */
  tolerates(reason: unknown): boolean;
}

/** The longest reason logged. */
const MAX_REASON = 300;

/** Handles every unhandled rejection of `source`: logged, then the process ends with code 1. */
export function handleRejections(source: RejectionSource, deps: RejectionDeps): void {
  source.on('unhandledRejection', (reason) => {
    const text = (reason instanceof Error ? reason.message : String(reason)).slice(0, MAX_REASON);
    if (deps.tolerates(reason)) {
      deps.log(`a cancelled Lighthouse run left a rejection behind: ${text}`);
      return;
    }
    deps.log(`an unhandled rejection ends the sidecar: ${text}`);
    deps.exit(1);
  });
}
