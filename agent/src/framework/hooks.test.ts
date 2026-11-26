import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { HOOK_MARKER, devtoolsInEvent, devtoolsOutEvent } from '../protocol';
import { loadPage } from '../test-page';
import { installReactDevtools } from './react-devtools';
import { installVueDevtools } from './vue-devtools';
import { DevToolsContextHookKeys, devtools } from '@vue/devtools-kit';

type Scope = Window & Record<string, unknown>;
const scope = (): Scope => window as unknown as Scope;

function send(framework: 'react' | 'vue', envelope: unknown): void {
  document.dispatchEvent(new CustomEvent(devtoolsInEvent(framework), { detail: JSON.stringify(envelope) }));
}

function outbox(framework: 'react' | 'vue'): unknown[] {
  const seen: unknown[] = [];
  document.addEventListener(devtoolsOutEvent(framework), (event) => {
    seen.push(JSON.parse((event as CustomEvent<string>).detail));
  });
  return seen;
}

/** What the React DevTools frontend does first: it answers `getSavedPreferences`, and only then
 *  the backend finishes its activation (its agent, which answers everything else). */
function playReactFrontend(): void {
  document.addEventListener(devtoolsOutEvent('react'), (event) => {
    const envelope: unknown = JSON.parse((event as CustomEvent<string>).detail);
    const message = (envelope as { d?: { event?: string } }).d;
    if (message?.event === 'getSavedPreferences') {
      send('react', { t: 'm', d: { event: 'savedPreferences', payload: { componentFilters: [] } } });
    }
  });
}

// The hooks define their globals for good (non-configurable), so each framework installs once
// per test file.
beforeEach(() => {
  loadPage('<!doctype html><html><head></head><body><div id="app"></div></body></html>');
});

/** Past the bridge's batch timer (100 ms) on the virtual clock: every batch due is out. */
const settleBridge = async (): Promise<void> => {
  await vi.advanceTimersByTimeAsync(150);
};

describe('installReactDevtools', () => {
  // The document stays the same object across `loadPage`; the frontend's part stays with it.
  beforeAll(() => playReactFrontend());
  // The bridge batches its messages on a timer: virtual time, so a wait is never too short.
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("installs React's hook with rBrowser's marker once and answers the panel only after connect", async () => {
    const seen = outbox('react');
    expect(installReactDevtools(window)).toBe(true);
    const hook = scope().__REACT_DEVTOOLS_GLOBAL_HOOK__ as Record<string, unknown>;
    expect(hook[HOOK_MARKER]).toBe(true);
    expect(hook.renderers).toBeInstanceOf(Map);
    expect(typeof hook.inject).toBe('function');
    expect(installReactDevtools(window)).toBe(false);
    send('react', { t: 'm', d: { event: 'getBackendVersion', payload: null } });
    expect(seen).toEqual([]);
    send('react', { t: 'connect' });
    // The bridge batches its messages; the backend asks for the saved preferences first.
    await settleBridge();
    expect(seen.some((e) => JSON.stringify(e).includes('getSavedPreferences'))).toBe(true);
    send('react', { t: 'disconnect' });
  });

  it('connects again after the panel closed, and once more when it opens without closing', async () => {
    const seen = outbox('react');
    const answers = (): unknown[] => seen.filter((e) => JSON.stringify(e).includes('"bridgeProtocol"'));
    const ask = (): void => send('react', { t: 'm', d: { event: 'getBridgeProtocol', payload: null } });
    const settle = settleBridge;
    // The hook is in place from the first test; the panel closed (disconnect): no bridge answers.
    expect(installReactDevtools(window)).toBe(false);
    send('react', { t: 'disconnect' });
    ask();
    await settle();
    expect(answers()).toEqual([]);
    // Opened again: a fresh bridge is activated (the frontend's preferences answered) and answers.
    send('react', { t: 'connect' });
    await settle();
    ask();
    await settle();
    expect(answers()).toHaveLength(1);
    // Opened once more without a disconnect (the panel's frame went away): the old bridge is
    // shut down first, so the new one answers alone.
    seen.length = 0;
    send('react', { t: 'connect' });
    await settle();
    ask();
    await settle();
    expect(answers()).toHaveLength(1);
    // Closed again: nothing answers.
    send('react', { t: 'disconnect' });
    seen.length = 0;
    ask();
    await settle();
    expect(answers()).toEqual([]);
  });
});

describe('installVueDevtools', () => {
  it("installs Vue's hook with rBrowser's marker and serves the RPC over the channel", async () => {
    const seen = outbox('vue');
    expect(installVueDevtools(window)).toBe(true);
    const hook = scope().__VUE_DEVTOOLS_GLOBAL_HOOK__ as Record<string, unknown>;
    expect(hook[HOOK_MARKER]).toBe(true);
    expect(hook.id).toBe('vue-devtools-next');
    expect(installVueDevtools(window)).toBe(false);
    send('vue', { t: 'connect' });
    // A birpc request: `heartbeat` answers true.
    send('vue', { t: 'm', d: { m: 'heartbeat', a: [], i: 'r1', t: 'q' } });
    await vi.waitFor(() => expect(seen).toContainEqual({ t: 'm', d: { t: 's', i: 'r1', r: true } }));
    send('vue', { t: 'disconnect' });
  });

  it('switches the timeline layers on for the panel and broadcasts timeline events', async () => {
    const seen = outbox('vue');
    // Detached (the first test's disconnect): the kit's high-performance mode, no tracking.
    expect(devtools.ctx.state.highPerfModeEnabled).toBe(true);
    send('vue', { t: 'connect' });
    expect(devtools.ctx.state.highPerfModeEnabled).toBe(false);
    send('vue', { t: 'm', d: { m: 'updateTimelineLayersState', a: [{ recordingState: true }], i: 'r2', t: 'q' } });
    await vi.waitFor(() => expect(seen).toContainEqual({ t: 'm', d: { t: 's', i: 'r2', r: null } }));
    expect(devtools.ctx.state.timelineLayersState.recordingState).toBe(true);
    // A plugin's event on the built-in mouse layer: the kit hands it to the server, which
    // broadcasts it to the panel as devtools-core does (`timeline-event-updated`, stringified).
    const event = { time: 12, title: 'click', data: { target: 'button' } };
    await devtools.ctx.hooks.callHook(DevToolsContextHookKeys.TIMELINE_EVENT_ADDED, {
      options: { layerId: 'mouse', event },
      plugin: { descriptor: { id: 'test-plugin' } },
    } as never);
    const broadcast = await vi.waitFor(() => {
      const found = seen.find((e) => JSON.stringify(e).includes('timeline-event-updated'));
      expect(found).toBeDefined();
      return found;
    });
    const text = JSON.stringify(broadcast);
    expect(text).toContain('mouse');
    expect(text).toContain('click');
    send('vue', { t: 'disconnect' });
    expect(devtools.ctx.state.highPerfModeEnabled).toBe(true);
  });
});
