// RB-094-23, review fix: "Restart now" is offered under a key that applies at the next start
// only once its change was saved - a write the core refused changed nothing to restart for.
import { describe, expect, it } from 'vitest';
import { createRestartOffers } from './restartOffers';
import type { SettingDef } from './schema';
import { realDefs } from './test/fakeCore';

function def(key: string, restart: boolean): SettingDef {
  const found = realDefs().find((candidate) => candidate.key === key);
  if (found === undefined) {
    throw new Error(`${key} is no key of the schemas`);
  }
  const schema = { ...found.schema };
  if (restart) {
    schema['x-restart'] = true;
  } else {
    delete schema['x-restart'];
  }
  return { ...found, schema };
}

describe('the restart offers', () => {
  it('offers the restart once a restart key was saved, not when the write was refused', async () => {
    const offers = createRestartOffers();
    const sandbox = def('engines.chromium.sandbox', true);
    await offers.after(sandbox, Promise.resolve(false));
    expect(offers.has(sandbox.key)).toBe(false);
    await offers.after(sandbox, Promise.resolve(true));
    expect(offers.has(sandbox.key)).toBe(true);
    await offers.after(sandbox, Promise.resolve(false));
    expect(offers.has(sandbox.key)).toBe(true);
  });

  it('offers nothing for a key that applies at once', async () => {
    const offers = createRestartOffers();
    const highlight = def('ui.highlightActiveView', false);
    await offers.after(highlight, Promise.resolve(true));
    expect(offers.has(highlight.key)).toBe(false);
  });
});
