// The names the agent and the core must spell alike (RB-094-10): each constant of protocol.ts held
// to its Rust twin in crates/rb-sync (event.rs, bus.rs), read from the source, so a rename on one
// side fails here and not in a view.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import * as protocol from './protocol';

const HERE = dirname(fileURLToPath(import.meta.url));
const RUST = ['event.rs', 'bus.rs'].map((file) => readFileSync(join(HERE, '../../crates/rb-sync/src', file), 'utf8')).join('\n');

function rustConst(name: string): string | undefined {
  return new RegExp(`pub const ${name}: &str = "([^"]*)";`).exec(RUST)?.[1];
}

describe('the protocol names', () => {
  it.each(['WORLD_NAME', 'BINDING_NAME', 'AGENT_GLOBAL', 'BOOT_GLOBAL', 'PAGE_API_GLOBAL', 'PAGE_BOOT_PARAM'] as const)(
    '%s is the one of crates/rb-sync',
    (name) => {
      expect(rustConst(name)).toBe(protocol[name]);
    },
  );

  it('the main-world bundle reads its state from the parameter the core wraps it in', () => {
    const main = readFileSync(join(HERE, 'page-api/main.ts'), 'utf8');
    expect(main).toContain(`declare const ${protocol.PAGE_BOOT_PARAM}: unknown;`);
  });
});
