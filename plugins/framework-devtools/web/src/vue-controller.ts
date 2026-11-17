// The Vue DevTools panel (RB-080-24, RB-080-31): the component tree of the active view and the
// selected component's state, the custom inspectors the kit's plugins register (Pinia's stores
// with state and getters, the router's routes), the router's current route and the kit's
// timeline, as tabs over the kit's RPC (`birpc` on the view's channel, the functions of
// `agent/src/framework/vue-functions.ts`). Reading and selecting only; hovering or selecting a
// component highlights it in the page, a double click scrolls to it. The panel follows the
// active view: it asks the component every `FOLLOW_MS`, disconnects the old view's backend and
// connects the new one's. The Timeline tab and its recording hang on the plugin setting
// `framework-devtools.vue.timeline`. `vue-panel.ts` starts it; this file is the testable part.

import { parse } from '@vue/devtools-kit';
import { createBirpc } from 'birpc';
import type { PanelBridge } from './panel-bridge';
import type { FrameworkReport } from './protocol';
import { nameOf, statusOf } from './status';
import { openViewChannel, watchProbe, type ViewChannel } from './view-channel';
import { idsToDepth, matching, prune, rows, stateRows, type Row, type StateRow, type TreeNode } from './vue-model';
import { renderStateTable, renderTabs, renderTreeList, type Tab } from './vue-render';
import { currentPath, currentRouteRows, routeRows, routerTree, type RouterTree } from './vue-router-model';
import { TimelineLog, eventRows, timelineRows } from './vue-timeline-model';
import { isObject } from '../../../web-common/src/is-object';

/** The kit's component inspector and the panel's own tabs. */
export const COMPONENTS = 'components';
export const ROUTER = 'router';
export const TIMELINE = 'timeline';
/** How often the panel asks which view is active, in ms. */
const FOLLOW_MS = 1500;
/** The levels open when a tree arrives. */
const OPEN_DEPTH = 3;
/** The longest inspector label and id taken from the page. */
const MAX_LABEL = 48;
/** The kit's own timeline layers, switched by these flags together with `recordingState`. */
const BUILT_IN_LAYERS = ['mouseEventEnabled', 'keyboardEventEnabled', 'componentEventEnabled', 'performanceEventEnabled'];
/** The setting that shows the Timeline tab and allows its recording. */
const TIMELINE_SETTING = 'framework-devtools.vue.timeline';

/** The backend's functions the panel calls (devtools-core's names). */
export interface ServerFunctions {
  heartbeat(): boolean;
  devtoolsState(): Promise<{ vueVersion?: string; timelineLayersState?: unknown }>;
  getInspectorTree(payload: { inspectorId: string; filter: string }): Promise<string | string[]>;
  getInspectorState(payload: { inspectorId: string; nodeId: string }): Promise<string | string[]>;
  getCustomInspector(): Promise<unknown>;
  getRouterInfo(): Promise<unknown>;
  updateTimelineLayersState(state: Record<string, boolean>): Promise<null>;
  highlightComponent(id: string): Promise<void>;
  unhighlight(): Promise<void>;
  scrollToComponent(id: string): Promise<void>;
  toggleClientConnected(state: boolean): void;
}

/** What the backend calls on the panel: its broadcasts. */
export interface ClientFunctions {
  emit(event: string, payload?: unknown): void;
}

/** A custom inspector a plugin of the page registered with the kit. */
interface Inspector {
  id: string;
  label: string;
  pluginId: string;
}

interface Session {
  view: number;
  channel: ViewChannel;
  rpc: ReturnType<typeof createBirpc<ServerFunctions, ClientFunctions>>;
}

const $ = (id: string): HTMLElement => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`no element #${id}`);
  return element;
};

function revive(text: string | string[]): unknown {
  return parse(Array.isArray(text) ? text.join('') : text, true);
}

/** The boolean switches of the kit's `timelineLayersState`, as the page has them. */
function layersOf(value: unknown): Record<string, boolean> {
  const out: Record<string, boolean> = {};
  if (!isObject(value)) return out;
  for (const [key, flag] of Object.entries(value)) if (typeof flag === 'boolean') out[key] = flag;
  return out;
}

/** The custom inspectors of the kit's answer, without the panel's own tab ids. */
export function inspectorsOf(value: unknown): Inspector[] {
  if (!Array.isArray(value)) return [];
  const out: Inspector[] = [];
  for (const item of value) {
    if (!isObject(item) || typeof item.id !== 'string' || typeof item.label !== 'string') continue;
    const id = item.id.slice(0, MAX_LABEL);
    if (id === '' || [COMPONENTS, ROUTER, TIMELINE].includes(id) || out.some((seen) => seen.id === id)) continue;
    const pluginId = typeof item.pluginId === 'string' ? item.pluginId.slice(0, MAX_LABEL) : '';
    out.push({ id, label: item.label.slice(0, MAX_LABEL), pluginId });
  }
  return out;
}

/** The panel's state and rendering. */
export class VuePanel {
  private readonly bridge: PanelBridge;
  private session: Session | null = null;
  private readonly reports = new Map<number, FrameworkReport>();
  private tab = COMPONENTS;
  private inspectors: Inspector[] = [];
  private tree: TreeNode[] = [];
  private expanded = new Set<string>();
  private selected: string | null = null;
  private state: StateRow[] = [];
  private router: unknown = null;
  private routes: RouterTree = { nodes: [], byId: new Map() };
  private readonly timeline = new TimelineLog();
  /** The page's timeline switches as read before the panel changed them, put back on leave. */
  private pageLayers: Record<string, boolean> | null = null;
  private recording = false;
  private treeRender: number | null = null;
  private filter = '';
  private version = '';
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor(bridge: PanelBridge) {
    this.bridge = bridge;
    bridge.onSettings(() => this.onSettings());
    watchProbe(bridge, (view, report) => {
      this.reports.set(view, report);
      this.renderStatus();
      if (this.session?.view === view && report.vue?.devtools) void this.load();
    });
    $('filter').addEventListener('input', (event) => {
      this.filter = (event.target as HTMLInputElement).value;
      this.renderTree();
    });
    $('refresh').addEventListener('click', () => {
      if (this.tab === TIMELINE) {
        this.timeline.clear();
        this.select(null);
        this.renderTree();
      } else {
        void this.load();
      }
    });
    $('tree').addEventListener('mouseleave', () => {
      if (this.tab === COMPONENTS) void this.session?.rpc.unhighlight().catch(() => undefined);
    });
  }

  /** Follows the active view from now on. */
  start(): void {
    this.renderTexts();
    this.bridge.requestSettings();
    const follow = async (): Promise<void> => {
      try {
        const answer = await this.bridge.call('active_view');
        const view = isObject(answer) && typeof answer.view === 'number' ? answer.view : null;
        await this.follow(view);
      } catch {
        // The host did not answer: the next tick asks again.
      }
    };
    void follow();
    this.timer = setInterval(() => void follow(), FOLLOW_MS);
  }

  /** Stops following (tests). */
  stop(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    if (this.treeRender !== null) cancelAnimationFrame(this.treeRender);
    this.treeRender = null;
  }

  private timelineAllowed(): boolean {
    return this.bridge.setting(TIMELINE_SETTING) !== false;
  }

  /** The texts or settings arrived: a Timeline tab switched off goes away, with its recording. */
  private onSettings(): void {
    if (this.tab === TIMELINE && !this.timelineAllowed()) {
      this.selectTab(COMPONENTS);
      return;
    }
    this.renderTexts();
  }

  private async follow(view: number | null): Promise<void> {
    if (this.session?.view === view) return;
    if (this.session) {
      const { channel, rpc } = this.session;
      await this.recordTimeline(false);
      this.session = null;
      rpc.$close();
      await channel.post({ t: 'disconnect' }).catch(() => undefined);
      channel.close();
    }
    this.reset();
    this.inspectors = [];
    this.timeline.clear();
    this.pageLayers = null;
    this.version = '';
    if (view !== null) {
      const channel = openViewChannel(this.bridge, view, 'vue');
      const rpc = createBirpc<ServerFunctions, ClientFunctions>(
        { emit: (event, payload) => this.onBroadcast(event, payload) },
        {
          post: (data: unknown) => void channel.post({ t: 'm', d: data }).catch(() => undefined),
          on: (fn: (data: unknown) => void) =>
            channel.on((envelope) => {
              if (envelope.t === 'm') fn(envelope.d);
            }),
          timeout: 10_000,
        },
      );
      this.session = { view, channel, rpc };
      await channel.post({ t: 'connect' }).catch(() => undefined);
      void this.load();
    }
    this.renderTexts();
  }

  /** Forgets the current tab's content. */
  private reset(): void {
    this.tree = [];
    this.expanded = new Set();
    this.selected = null;
    this.state = [];
    this.router = null;
    this.routes = { nodes: [], byId: new Map() };
  }

  private onBroadcast(event: string, payload: unknown): void {
    if (event === 'inspector-tree-updated' && typeof payload === 'string') {
      const data = revive(payload);
      if (isObject(data) && data.inspectorId === this.tab && Array.isArray(data.rootNodes)) {
        this.setTree(data.rootNodes as TreeNode[]);
      }
    } else if (event === 'inspector-state-updated' && typeof payload === 'string') {
      const data = revive(payload);
      if (isObject(data) && data.inspectorId === this.tab && data.nodeId === this.selected) {
        this.state = stateRows(data.state);
        this.renderState();
      }
    } else if (event === 'inspector-updated') {
      this.setInspectors(payload);
    } else if (event === 'router-info-updated') {
      this.setRouter(payload);
    } else if (event === 'timeline-event-updated' && typeof payload === 'string') {
      if (this.recording && this.timeline.push(revive(payload)) && this.tab === TIMELINE) this.scheduleTree();
    } else if (event === 'devtools-state-updated' || event === 'active-app-updated') {
      void this.load();
    }
  }

  /** Asks the backend for the version, the custom inspectors and the current tab's content. */
  private async load(): Promise<void> {
    const session = this.session;
    if (!session) return;
    try {
      session.rpc.toggleClientConnected(true);
      const state = await session.rpc.devtoolsState();
      this.version = state.vueVersion ?? '';
      if (this.pageLayers === null) this.pageLayers = layersOf(state.timelineLayersState);
      this.setInspectors(await session.rpc.getCustomInspector());
      await this.loadTab(session);
    } catch {
      // The backend is not there yet (no hook, no app): the status line says so.
    }
    this.renderStatus();
  }

  /** Loads the tab shown; an answer that arrives after a switch of view or tab is dropped. */
  private async loadTab(session: Session): Promise<void> {
    const tab = this.tab;
    if (tab === TIMELINE) {
      await this.recordTimeline(true);
    } else if (tab === ROUTER) {
      const info = await session.rpc.getRouterInfo();
      if (this.session !== session || this.tab !== tab) return;
      this.setRouter(info);
    } else {
      const text = await session.rpc.getInspectorTree({ inspectorId: tab, filter: '' });
      if (this.session !== session || this.tab !== tab) return;
      const tree = revive(text);
      this.setTree(Array.isArray(tree) ? (tree as TreeNode[]) : []);
    }
  }

  private setInspectors(value: unknown): void {
    this.inspectors = inspectorsOf(value);
    if (![COMPONENTS, ROUTER, TIMELINE].includes(this.tab) && !this.inspectors.some((i) => i.id === this.tab)) {
      // The inspector shown is gone (its app unmounted): back to the components, loaded.
      this.tab = COMPONENTS;
      this.reset();
      this.renderTexts();
      const session = this.session;
      if (session) void this.loadTab(session).catch(() => undefined);
      return;
    }
    this.renderTabs();
  }

  private setTree(tree: TreeNode[]): void {
    const first = this.tree.length === 0;
    this.tree = tree;
    if (first) this.expanded = new Set(idsToDepth(tree, OPEN_DEPTH));
    this.renderTree();
  }

  private setRouter(info: unknown): void {
    this.router = info;
    if (this.tab !== ROUTER) return;
    this.routes = routerTree(info);
    if (this.selected !== null && !this.routes.byId.has(this.selected)) this.selected = null;
    this.state = this.selected === null ? [] : routeRows(this.routes.byId.get(this.selected));
    this.renderTexts();
  }

  /** Switches the kit's timeline recording on - its own layers and every plugin's - or off,
   *  putting the page's own switches back: the kit persists them in the page's localStorage,
   *  where the official client reads them too. */
  private async recordTimeline(on: boolean): Promise<void> {
    const session = this.session;
    if (!session || this.recording === on || (on && !this.timelineAllowed())) return;
    this.recording = on;
    const state: Record<string, boolean> = { recordingState: on };
    if (on) {
      for (const flag of BUILT_IN_LAYERS) state[flag] = true;
      for (const inspector of this.inspectors) if (inspector.pluginId !== '') state[inspector.pluginId] = true;
    } else {
      Object.assign(state, this.pageLayers ?? {}, { recordingState: this.pageLayers?.recordingState ?? false });
    }
    await session.rpc.updateTimelineLayersState(state).catch(() => undefined);
  }

  /** The tabs: components, the custom inspectors, the router and the timeline. */
  private tabs(): Tab[] {
    const t = (key: string): string => this.bridge.text(key);
    return [
      { id: COMPONENTS, label: t('panel.components') },
      ...this.inspectors.map((inspector) => ({ id: inspector.id, label: inspector.label })),
      { id: ROUTER, label: t('panel.router') },
      ...(this.timelineAllowed() ? [{ id: TIMELINE, label: t('panel.timeline') }] : []),
    ];
  }

  /** Shows `id`'s tab and loads it. */
  selectTab(id: string): void {
    if (id === this.tab || !this.tabs().some((tab) => tab.id === id)) return;
    this.tab = id;
    this.reset();
    this.renderTexts();
    const session = this.session;
    if (!session) return;
    void (async () => {
      if (id !== TIMELINE) await this.recordTimeline(false);
      await this.loadTab(session);
    })().catch(() => undefined);
  }

  private select(id: string | null): void {
    this.selected = id;
    this.state = [];
    this.renderTree();
    this.renderState();
    if (id === null) return;
    if (this.tab === ROUTER) {
      this.state = routeRows(this.routes.byId.get(id));
    } else if (this.tab === TIMELINE) {
      const entry = this.timeline.get(Number(id));
      this.state = entry === undefined ? [] : eventRows(entry);
    } else {
      void this.loadState(id);
      return;
    }
    this.renderState();
  }

  private async loadState(id: string): Promise<void> {
    const session = this.session;
    if (!session) return;
    const inspectorId = this.tab;
    try {
      if (inspectorId === COMPONENTS) void session.rpc.highlightComponent(id).catch(() => undefined);
      const text = await session.rpc.getInspectorState({ inspectorId, nodeId: id });
      if (this.selected !== id || this.tab !== inspectorId) return;
      this.state = stateRows(revive(text));
    } catch {
      this.state = [];
    }
    this.renderState();
  }

  private renderTabs(): void {
    renderTabs($('tabs'), this.tabs(), this.tab, (id) => this.selectTab(id));
  }

  private renderTexts(): void {
    const t = (key: string, params?: Record<string, string | number>): string => this.bridge.text(key, params);
    document.title = t('panels.vue');
    $('title').textContent = t('panels.vue');
    $('filter').setAttribute('placeholder', t('panel.filter'));
    $('refresh').textContent = t(this.tab === TIMELINE ? 'panel.clear' : 'panel.refresh');
    const path = this.tab === ROUTER ? currentPath(this.router) : null;
    $('pane-title').textContent =
      path === null ? (this.tabs().find((tab) => tab.id === this.tab)?.label ?? '') : t('panel.route', { path });
    $('state-title').textContent = t('panel.state');
    this.renderTabs();
    this.renderStatus();
    this.renderTree();
    this.renderState();
  }

  private renderStatus(): void {
    const view = this.session?.view ?? null;
    const status = statusOf('vue', view, view === null ? null : (this.reports.get(view) ?? null));
    const params = { framework: nameOf('vue'), view: view ?? '', version: this.version };
    $('status').textContent = this.bridge.text(`panel.${status}`, params);
    $('status').dataset.status = status;
  }

  /** The rows of the current tab under the filter. */
  private shownRows(): Row[] {
    if (this.tab === TIMELINE) return timelineRows(this.timeline.entries(), this.filter);
    const nodes = this.tab === ROUTER ? this.routes.nodes : this.tree;
    const kept = prune(nodes, matching(nodes, this.filter));
    const open = this.filter.trim() === '' ? this.expanded : new Set(matching(nodes, this.filter));
    return rows(kept, open);
  }

  /** Renders the tree pane once per frame, however many events arrive in it. */
  private scheduleTree(): void {
    if (this.treeRender !== null) return;
    this.treeRender = requestAnimationFrame(() => {
      this.treeRender = null;
      this.renderTree();
    });
  }

  private renderTree(): void {
    const components = this.tab === COMPONENTS;
    const empty = this.tab === TIMELINE ? 'panel.noEvents' : this.tab === ROUTER ? 'panel.noRoute' : components ? 'panel.empty' : 'panel.noNodes';
    renderTreeList($('tree'), this.shownRows(), {
      selected: this.selected,
      emptyText: this.bridge.text(empty),
      logType: this.tab === TIMELINE ? (id) => this.timeline.get(Number(id))?.logType : undefined,
      onToggle: (id) => {
        if (this.expanded.has(id)) this.expanded.delete(id);
        else this.expanded.add(id);
        this.renderTree();
      },
      onSelect: (id) => this.select(id),
      onHover: components ? (id) => void this.session?.rpc.highlightComponent(id).catch(() => undefined) : undefined,
      onOpen: components ? (id) => void this.session?.rpc.scrollToComponent(id).catch(() => undefined) : undefined,
    });
  }

  private renderState(): void {
    const t = (key: string): string => this.bridge.text(key);
    let entries = this.state;
    let empty = this.tab === COMPONENTS ? 'panel.noSelection' : 'panel.noNodeSelection';
    if (this.tab === ROUTER) {
      if (this.selected === null) entries = currentRouteRows(this.router);
      empty = 'panel.noRoute';
    } else if (this.tab === TIMELINE) {
      empty = 'panel.noEvent';
    }
    renderStateTable($('state'), entries, t(empty), (section) => {
      const named = t(`panel.sections.${section}`);
      return named === `panel.sections.${section}` ? section : named;
    });
  }
}
