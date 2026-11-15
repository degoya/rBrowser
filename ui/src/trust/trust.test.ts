import { describe, expect, it } from 'vitest';
import { createTrust, kindKey } from './trust';
import { fakeTrustCore, ROOT } from './testCore';

describe('the trust store', () => {
  it('keeps the prompt the core sends and grants only through it', async () => {
    const core = fakeTrustCore();
    const trust = createTrust(core.client);
    trust.start();
    expect(await trust.decide('trust')).toBe(false);
    expect(core.sent).toHaveLength(0);

    await trust.refresh();
    expect(trust.prompt.value?.promptId).toBe(7);
    expect(trust.info.value?.current?.trusted).toBe(false);

    expect(await trust.decide('trust')).toBe(true);
    expect(core.sent).toContainEqual({ type: 'trustDecision', promptId: 7, decision: 'trust' });
    expect(trust.prompt.value).toBeNull();
    expect(trust.info.value?.current?.trusted).toBe(true);
    expect(trust.info.value?.projects).toHaveLength(1);
    trust.stop();
  });

  it('revokes and is asked again', async () => {
    const core = fakeTrustCore();
    const trust = createTrust(core.client);
    trust.start();
    await trust.refresh();
    await trust.decide('trust');
    await trust.revoke(ROOT);
    expect(trust.info.value?.current?.trusted).toBe(false);
    expect(trust.info.value?.projects).toHaveLength(0);
    core.prompt();
    expect(trust.prompt.value?.blocked).toHaveLength(2);
  });

  it('opens the dialog through the command and reports a refusal', async () => {
    const core = fakeTrustCore();
    const trust = createTrust(core.client);
    expect(await trust.openDialog()).toBe(true);
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'trust.show', args: {} });
    trust.prompt.value = { promptId: 99, projectRoot: ROOT, blocked: [] };
    expect(await trust.decide('trust')).toBe(false);
    expect(trust.failed.value).toBe(true);
  });

  it('names every kind with an i18n key', () => {
    expect(kindKey({ kind: 'mocks', path: '.rbrowser/mocks/a.json' })).toBe('trust.kind.mocks');
  });
});
