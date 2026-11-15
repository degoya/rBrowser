// The Vue panel against a fixture of what the kit answers for an app with Pinia and vue-router
// (RB-080-31): the backend is a birpc server over a fake plugin bridge, its answers the kit's
// own `stringify` of a component tree, a Pinia store with state and getters, and router info.

import { stringify } from '@vue/devtools-kit';
import { createBirpc } from 'birpc';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import en from '../../locales/en.json';
import vueHtml from '../../panel/vue.html?raw';
import type { PanelBridge } from './panel-bridge';
import type { Post } from './protocol';
import { VuePanel, inspectorsOf, type ClientFunctions, type ServerFunctions } from './vue-controller';

const PREFIX = 'plugin.framework-devtools.';

/** The plugin's texts, flattened as the panels window sends them. */
function strings(): Record<string, string> {
  const out: Record<string, string> = {};
  const walk = (value: unknown, path: string): void => {
    if (typeof value === 'string') out[path] = value;
    else if (typeof value === 'object' && value !== null) {
      for (const [key, child] of Object.entries(value)) walk(child, path === '' ? key : `${path}.${key}`);
    }
  };
  walk(en, '');
  return out;
}

const components = [
  { id: 'root', label: 'App', children: [{ id: 'c1', label: 'Counter', tags: [] }] },
];
const stores = [{ id: 'counter', label: 'counter', tags: [{ label: 'store' }] }];
const storeState = {
  state: [{ key: 'count', value: 1, editable: true }],
  getters: [{ key: 'double', value: 2 }],
};
const routerInfo = {
  currentRoute: { path: '/about', fullPath: '/about?x=1', name: 'about', hash: '', params: {}, query: { x: '1' }, matched: [] },
  routes: [
    { path: '/', name: 'home' },
    { path: '/about', name: 'about', meta: { title: 'About' } },
  ],
};

interface Fixture {
  bridge: PanelBridge;
  backend: ReturnType<typeof createBirpc<ClientFunctions, ServerFunctions>>;
  calls: string[];
  layers: Record<string, boolean>[];
  /** The probe's report of view 1, as the page script relays it. */
  probe(): void;
  /** Holds the Pinia tree's answer back until `release`. */
  hold: { pinia: boolean };
  release(): void;
}

/** The kit's timeline switches as the page has them before the panel touches them. */
const pageLayers = { recordingState: false, mouseEventEnabled: false, keyboardEventEnabled: true, 'dev.esm.pinia': false };

/** A fake panels window and the view's backend behind it. */
function fixture(settings: Record<string, unknown> = {}, layersState: unknown = pageLayers): Fixture {
  const texts = strings();
  const calls: string[] = [];
  const layers: Record<string, boolean>[] = [];
  const hold = { pinia: false };
  let release = (): void => undefined;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const eventListeners = new Set<(view: number | null, message: unknown) => void>();
  const fromPanel: ((data: unknown) => void)[] = [];
  const toPanel = (envelope: unknown): void => {
    for (const listener of eventListeners) listener(1, { framework: 'vue', text: JSON.stringify(envelope) });
  };
  const functions: ServerFunctions = {
    heartbeat: () => true,
    devtoolsState: async () => ({ vueVersion: '3.5.13', timelineLayersState: layersState }),
    getInspectorTree: async ({ inspectorId }) => {
      calls.push(`tree ${inspectorId}`);
      if (inspectorId === 'pinia' && hold.pinia) await held;
      return stringify(inspectorId === 'pinia' ? stores : inspectorId === 'components' ? components : []);
    },
    getInspectorState: async ({ inspectorId, nodeId }) => {
      calls.push(`state ${inspectorId} ${nodeId}`);
      return stringify(inspectorId === 'pinia' && nodeId === 'counter' ? storeState : { props: [{ key: 'n', value: 3 }] });
    },
    getCustomInspector: async () => [
      { id: 'pinia', label: 'Pinia 🍍', pluginId: 'dev.esm.pinia' },
      { id: 'routes', label: 'Routes', pluginId: 'org.vuejs.router' },
    ],
    getRouterInfo: async () => routerInfo,
    updateTimelineLayersState: async (state) => {
      layers.push(state);
      return null;
    },
    highlightComponent: async (id) => {
      calls.push(`highlight ${id}`);
    },
    unhighlight: async () => undefined,
    scrollToComponent: async () => undefined,
    toggleClientConnected: () => undefined,
  };
  const backend = createBirpc<ClientFunctions, ServerFunctions>(functions, {
    post: (data: unknown) => toPanel({ t: 'm', d: data }),
    on: (fn: (data: unknown) => void) => {
      fromPanel.push(fn);
    },
  });
  const bridge: PanelBridge = {
    async call(tool, args) {
      if (tool === 'active_view') return { view: 1 };
      if (tool === 'send') {
        const post = args as Post & { view: number };
        calls.push(`send ${post.envelope.t}`);
        if (post.envelope.t === 'm') for (const fn of fromPanel) fn(post.envelope.d);
        return {};
      }
      throw new Error('unknown tool');
    },
    text: (key, params = {}) =>
      (texts[PREFIX + key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? '')),
    setting: (key) => settings[key],
    onSettings: () => () => undefined,
    onEvent: (listener) => {
      eventListeners.add(listener);
      return () => eventListeners.delete(listener);
    },
    requestSettings: () => undefined,
  };
  const probe = (): void => {
    const text = JSON.stringify({ vue: { hook: true, devtools: true, version: '3.5.13' } });
    for (const listener of eventListeners) listener(1, { framework: 'probe', text });
  };
  return { bridge, backend, calls, layers, probe, hold, release };
}

const el = (id: string): HTMLElement => {
  const element = document.getElementById(id);
  if (!element) throw new Error(id);
  return element;
};
const tabs = (): HTMLButtonElement[] => [...el('tabs').querySelectorAll('button')];
const tab = (id: string): HTMLButtonElement => {
  const button = tabs().find((b) => b.dataset.tab === id);
  if (!button) throw new Error(`no tab ${id}`);
  return button;
};
const treeLabels = (): string[] => [...el('tree').querySelectorAll('.label')].map((label) => label.textContent ?? '');
const stateText = (): string => el('state').textContent ?? '';
const rowOf = (label: string): HTMLElement => {
  const row = [...el('tree').querySelectorAll('li')].find((li) => li.querySelector('.label')?.textContent === label);
  if (!row) throw new Error(`no row ${label}`);
  return row;
};

let panel: VuePanel | null = null;

beforeEach(() => {
  // The panel's page without its script and stylesheet, which happy-dom would try to load.
  const page = new DOMParser().parseFromString(vueHtml.replace(/<(link|script)\b[^>]*>/g, ''), 'text/html');
  document.body.innerHTML = page.body.innerHTML;
});

afterEach(() => {
  panel?.stop();
  panel = null;
});

describe('VuePanel with Pinia and a router', () => {
  it('shows the stores with their state and getters, the routes and the current route', async () => {
    const { bridge, calls, probe } = fixture();
    panel = new VuePanel(bridge);
    panel.start();
    probe();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    expect(tabs().map((b) => b.textContent)).toEqual(['Components', 'Pinia 🍍', 'Routes', 'Router', 'Timeline']);
    expect(el('status').textContent).toBe('Connected to Vue 3.5.13 in view 1.');
    expect(calls).toContain('send connect');

    tab('pinia').click();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['counter']));
    expect(el('pane-title').textContent).toBe('Pinia 🍍');
    expect(stateText()).toBe('Select an entry to see its state.');
    rowOf('counter').click();
    await vi.waitFor(() => expect(stateText()).toContain('count'));
    expect([...el('state').querySelectorAll('h3')].map((h) => h.textContent)).toEqual(['state', 'getters']);
    expect([...el('state').querySelectorAll('.row')].map((r) => r.textContent)).toEqual(['count1', 'double2']);
    expect(calls).toContain('state pinia counter');
    expect(calls.some((call) => call.startsWith('highlight'))).toBe(false);

    tab('router').click();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['/', '/about']));
    expect(el('pane-title').textContent).toBe('Current route: /about');
    expect([...el('state').querySelectorAll('h3')].map((h) => h.textContent)).toEqual(['Current route', 'Query']);
    expect(stateText()).toContain('"/about?x=1"');
    rowOf('/about').click();
    expect([...el('state').querySelectorAll('h3')].map((h) => h.textContent)).toEqual(['Route', 'Meta']);
    expect(stateText()).toContain('"About"');
  });

  it('records the timeline while its tab is open and lists the events', async () => {
    const { bridge, backend, layers } = fixture();
    panel = new VuePanel(bridge);
    panel.start();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    tab('timeline').click();
    await vi.waitFor(() => expect(layers).toHaveLength(1));
    expect(layers[0]).toEqual({
      recordingState: true,
      mouseEventEnabled: true,
      keyboardEventEnabled: true,
      componentEventEnabled: true,
      performanceEventEnabled: true,
      'dev.esm.pinia': true,
      'org.vuejs.router': true,
    });
    expect(el('tree').textContent).toContain('No events yet.');
    expect(el('refresh').textContent).toBe('Clear');
    void backend.emit(
      'timeline-event-updated',
      stringify({
        layerId: 'pinia:mutations',
        event: { time: 1_700_000_000_000, title: 'counter', subtitle: 'increment', data: { key: 'count', newValue: 2 } },
      }),
    );
    await vi.waitFor(() => expect(treeLabels()).toEqual(['counter: increment']));
    rowOf('counter: increment').click();
    expect([...el('state').querySelectorAll('h3')].map((h) => h.textContent)).toEqual(['Event', 'Data']);
    expect(stateText()).toContain('newValue2');
    el('refresh').click();
    expect(el('tree').textContent).toContain('No events yet.');
    // Leaving the tab puts the page's switches back as they were (the kit persists them in
    // the page's localStorage, where the official client reads them too).
    tab('components').click();
    await vi.waitFor(() => expect(layers).toHaveLength(2));
    expect(layers[1]).toEqual(pageLayers);
  });

  it('switches only the recording off when the page said nothing about its layers', async () => {
    const { bridge, layers } = fixture({}, null);
    panel = new VuePanel(bridge);
    panel.start();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    tab('timeline').click();
    await vi.waitFor(() => expect(layers).toHaveLength(1));
    tab('router').click();
    await vi.waitFor(() => expect(layers).toHaveLength(2));
    expect(layers[1]).toEqual({ recordingState: false });
  });

  it('has no Timeline tab and records nothing while the setting is off', async () => {
    const { bridge, layers, backend } = fixture({ 'framework-devtools.vue.timeline': false });
    panel = new VuePanel(bridge);
    panel.start();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    expect(tabs().map((b) => b.dataset.tab)).toEqual(['components', 'pinia', 'routes', 'router']);
    void backend.emit('timeline-event-updated', stringify({ layerId: 'mouse', event: { time: 1, title: 'click' } }));
    tab('router').click();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['/', '/about']));
    expect(layers).toEqual([]);
  });

  it("shows the tab chosen last, not a slower answer of the tab left", async () => {
    const { bridge, hold, release } = fixture();
    hold.pinia = true;
    panel = new VuePanel(bridge);
    panel.start();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    tab('pinia').click();
    tab('components').click();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    release();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(treeLabels()).toEqual(['App', 'Counter']);
    expect(el('pane-title').textContent).toBe('Components');
  });

  it('falls back to the components, loaded, when the inspector shown goes away', async () => {
    const { bridge, backend, calls } = fixture();
    panel = new VuePanel(bridge);
    panel.start();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    tab('pinia').click();
    await vi.waitFor(() => expect(treeLabels()).toEqual(['counter']));
    const before = calls.filter((call) => call === 'tree components').length;
    void backend.emit('inspector-updated', [{ id: 'routes', label: 'Routes', pluginId: 'org.vuejs.router' }]);
    await vi.waitFor(() => expect(treeLabels()).toEqual(['App', 'Counter']));
    expect(tabs().map((b) => b.dataset.tab)).toEqual(['components', 'routes', 'router', 'timeline']);
    expect(calls.filter((call) => call === 'tree components').length).toBe(before + 1);
  });
});

describe('inspectorsOf', () => {
  it("takes the kit's custom inspectors, not the panel's own tab ids or junk", () => {
    expect(
      inspectorsOf([
        { id: 'pinia', label: 'Pinia', pluginId: 'dev.esm.pinia' },
        { id: 'router', label: 'Clash', pluginId: 'x' },
        { id: 'pinia', label: 'Twice', pluginId: 'x' },
        { id: 7, label: 'x' },
        'junk',
        { id: 'x'.repeat(60), label: 'y'.repeat(60) },
      ]),
    ).toEqual([
      { id: 'pinia', label: 'Pinia', pluginId: 'dev.esm.pinia' },
      { id: 'x'.repeat(48), label: 'y'.repeat(48), pluginId: '' },
    ]);
    expect(inspectorsOf(null)).toEqual([]);
  });
});
