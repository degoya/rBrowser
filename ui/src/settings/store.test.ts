import { describe, expect, it } from 'vitest';
import { createSettingsStore } from './store';
import { createFakeCore } from './test/fakeCore';
import { settle } from './test/mountPage';

describe('the settings store', () => {
  it('loads the schema once and the values', async () => {
    const core = createFakeCore();
    const store = createSettingsStore(core.client);
    await store.start();
    expect(store.schema.value?.defs.length).toBeGreaterThan(50);
    expect(store.values.value.get('sync.scroll')?.origin).toBe('default');
    expect(core.sent[0]).toEqual({ type: 'getSettings', includeSchema: true });
  });

  it('writes, keeps the reasons of a refusal at the key, and resets from the origin scope', async () => {
    const core = createFakeCore({ origins: { 'sync.scroll': 'local' } });
    const store = createSettingsStore(core.client);
    await store.start();
    expect(await store.set('network.ignoreCertificateErrors', true, 'project')).toBe(false);
    expect(store.issues.value['network.ignoreCertificateErrors']?.[0]?.messageKey).toBe(
      'settings.rejected.local_only',
    );
    expect(await store.set('network.ignoreCertificateErrors', true, 'local')).toBe(true);
    expect(store.issues.value['network.ignoreCertificateErrors']).toBeUndefined();
    const def = store.schema.value!.defs.find((d) => d.key === 'sync.scroll')!;
    expect(await store.reset(def)).toBe(true);
    expect(core.sent).toContainEqual({ type: 'setSetting', key: 'sync.scroll', value: null, scope: 'local' });
    expect(store.values.value.get('sync.scroll')?.origin).toBe('default');
  });

  it('reads the values again when another window changed them', async () => {
    const core = createFakeCore();
    const store = createSettingsStore(core.client);
    await store.start();
    core.values.set('ui.theme', { key: 'ui.theme', value: 'dark', origin: 'user' });
    core.emit({ type: 'settingsChanged', keys: ['ui.theme'] });
    await settle();
    expect(store.values.value.get('ui.theme')?.value).toBe('dark');
  });

  it('gets a picked path from the event after the ack', async () => {
    const core = createFakeCore();
    const store = createSettingsStore(core.client);
    expect(await store.pickPath('file', 'settings.key.engines.nodePath.label')).toBe('C:\\node\\node.exe');
  });
});
