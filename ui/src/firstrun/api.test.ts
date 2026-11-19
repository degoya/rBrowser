import { describe, expect, it } from 'vitest';
import { createSetupApi } from './api';
import { fakeCore, SAMPLE_PLUGINS } from './testing';

describe('the setup wizard talks to the core', () => {
  it('asks info and closes', async () => {
    const core = fakeCore((message) =>
      message.type === 'setup' && message.request.op === 'info'
        ? { type: 'setup', answer: { op: 'info', enabled: true } }
        : undefined,
    );
    const api = createSetupApi(core.client);
    expect(await api.info()).toEqual({ op: 'info', enabled: true });
    await api.close();
    expect(core.sent.at(-1)).toEqual({ type: 'setup', request: { op: 'close' } });
  });

  it('numbers each step and takes only the answer with its number', async () => {
    const core = fakeCore();
    const api = createSetupApi(core.client);
    const first = api.preview({ op: 'preview', root: 'C:/old', template: 'default' });
    const second = api.preview({ op: 'preview', root: 'C:/site', template: 'default' });
    await Promise.resolve();
    const ids = core.sent.map((message) => (message.type === 'setup' && 'requestId' in message.request ? message.request.requestId : -1));
    expect(ids).toHaveLength(2);
    expect(ids[1]).toBeGreaterThan(ids[0] ?? 0);
    const [oldId, newId] = ids as [number, number];
    // The newer answer first, then the stale one: each lands with its own request.
    core.emit({
      type: 'setup',
      answer: { op: 'proposal', requestId: newId, proposalId: 2, root: 'C:/site', config: { version: 1 }, source: 'default' },
    });
    // An answer to a request this page did not send lands nowhere.
    core.emit({
      type: 'setup',
      answer: { op: 'proposal', requestId: newId + 50, proposalId: 9, root: 'C:/x', config: {}, source: 'default' },
    });
    core.emit({
      type: 'setup',
      answer: { op: 'proposal', requestId: oldId, proposalId: 1, root: 'C:/old', config: { version: 1 }, source: 'default' },
    });
    expect(await second).toMatchObject({ op: 'proposal', proposalId: 2 });
    expect(await first).toMatchObject({ op: 'proposal', proposalId: 1 });
  });

  it('takes a failure with its number as the answer', async () => {
    const core = fakeCore();
    const api = createSetupApi(core.client);
    const pending = api.write(3);
    await Promise.resolve();
    const sent = core.sent.at(-1);
    const id = sent?.type === 'setup' && sent.request.op === 'write' ? sent.request.requestId : -1;
    expect(sent).toEqual({ type: 'setup', request: { op: 'write', requestId: id, proposalId: 3 } });
    core.emit({ type: 'setup', answer: { op: 'failed', requestId: id + 100, step: 'write', messageKey: 'x' } });
    core.emit({ type: 'setup', answer: { op: 'failed', requestId: id, step: 'write', messageKey: 'firstrun.project.error.exists' } });
    expect(await pending).toMatchObject({ op: 'failed', messageKey: 'firstrun.project.error.exists' });
  });

  it('takes the recommended plugins and removes unticked ones through RB-091-04', async () => {
    const core = fakeCore((message) =>
      message.type === 'pluginPanel'
        ? { type: 'pluginPanel', message: { value: { results: [{ id: 'video', ok: true }], restart: true } } }
        : undefined,
    );
    const api = createSetupApi(core.client);
    await api.applyRecommended();
    expect(core.sent.at(-1)).toEqual({ type: 'pluginPanel', request: { op: 'applyRecommended' } });
    const removed = await api.removeChosen(['video']);
    expect(core.sent.at(-1)).toEqual({ type: 'pluginPanel', request: { op: 'removeChosen', ids: ['video'] } });
    expect(removed.results).toEqual([{ id: 'video', ok: true, origin: 'bundle' }]);
  });

  it('asks RB-091-04 for the plugins and installs the choice with its progress', async () => {
    const core = fakeCore((message) => {
      if (message.type !== 'pluginPanel') return undefined;
      const op = (message.request as { op: string }).op;
      if (op === 'available') {
        return { type: 'pluginPanel', message: { value: { offline: false, choicePending: true, plugins: SAMPLE_PLUGINS } } };
      }
      core.emit({
        type: 'pluginPanel',
        message: { pluginInstallProgress: { id: 'seo', done: 1, total: 1, state: 'installed' } } as never,
      });
      return {
        type: 'pluginPanel',
        message: { value: { results: [{ id: 'seo', ok: true, origin: 'bundle' }], restart: true } },
      };
    });
    const api = createSetupApi(core.client);
    const available = await api.available();
    expect(available.plugins).toHaveLength(SAMPLE_PLUGINS.length);
    const seen: number[] = [];
    const outcome = await api.installChosen(['seo'], (progress) => seen.push(progress.done));
    expect(core.sent.at(-1)).toEqual({ type: 'pluginPanel', request: { op: 'installChosen', ids: ['seo'] } });
    expect(seen).toEqual([1]);
    expect(outcome.results).toEqual([{ id: 'seo', ok: true, origin: 'bundle' }]);
  });

  it('sets the theme through the registry command', async () => {
    const core = fakeCore();
    await createSetupApi(core.client).setTheme('dark');
    expect(core.sent.at(-1)).toEqual({ type: 'runCommand', id: 'ui.theme.set', args: { theme: 'dark' } });
  });
});
