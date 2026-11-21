// RB-092-06: an install, update or removal takes effect live; the core says so with the
// `pluginPanel` event `pluginsChanged`, and Settings > Plugins asks for its list again.
import { describe, expect, it } from 'vitest';
import type { IpcClient } from '../../ipc/client';
import type { CoreToShell } from '../../ipc/protocol';
import { createInstalledLoader, isPluginsChanged, refreshOnChange } from './loader';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A client that answers `installed` with `rows` plugins and lets the test send events. */
function fakeClient() {
  let listener: ((message: CoreToShell) => void) | null = null;
  let asked = 0;
  const client = {
    async expect() {
      asked += 1;
      return { type: 'pluginPanel', message: { loading: false, plugins: [] } };
    },
    on(next: (message: CoreToShell) => void) {
      listener = next;
      return () => {
        listener = null;
      };
    },
  };
  return {
    client: client as unknown as IpcClient,
    asked: () => asked,
    emit: (message: CoreToShell) => listener?.(message),
    listening: () => listener !== null,
  };
}

const changed: CoreToShell = { type: 'pluginPanel', message: { pluginsChanged: true } };

describe('the installed plugins follow a live change', () => {
  it('tells the change event from the other plugin panel messages', () => {
    expect(isPluginsChanged(changed)).toBe(true);
    expect(isPluginsChanged({ type: 'pluginPanel', message: { panelsChanged: true } })).toBe(false);
    expect(isPluginsChanged({ type: 'pluginPanel', message: { value: null } })).toBe(false);
  });

  it('asks again on the event once the list was asked for, and stops listening when told', async () => {
    const fake = fakeClient();
    const loader = createInstalledLoader(fake.client);
    const stop = refreshOnChange(fake.client, loader);
    // Never asked yet (Plugins not opened): the event asks nothing.
    fake.emit(changed);
    await settle();
    expect(fake.asked()).toBe(0);

    loader.refresh();
    await settle();
    expect(fake.asked()).toBe(1);
    fake.emit({ type: 'pluginPanel', message: { panelsChanged: true } });
    await settle();
    expect(fake.asked()).toBe(1);
    fake.emit(changed);
    await settle();
    expect(fake.asked()).toBe(2);

    stop();
    expect(fake.listening()).toBe(false);
    loader.stop();
  });
});
