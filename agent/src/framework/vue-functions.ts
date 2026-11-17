// The RPC functions the Vue DevTools panel calls in the page (RB-080-24): the part of
// `@vue/devtools-core`'s server function table the panel needs, written over `@vue/devtools-kit`
// directly - devtools-core imports `vue` for its client-side helpers, which would put a second
// Vue runtime into every page. Names and payloads follow devtools-core, so the panel speaks the
// official client's protocol: a `stringify`-ed tree and state (the kit's own serialiser, which
// keeps circular references and special values), the broadcast events of
// `DevToolsMessagingEvents` on every update.

import {
  DevToolsContextHookKeys,
  DevToolsMessagingHookKeys,
  devtools,
  devtoolsRouterInfo,
  getActiveInspectors,
  getInspector,
  getInspectorActions,
  getInspectorInfo,
  getInspectorNodeActions,
  getRpcServer,
  stringify,
  toggleClientConnected,
  updateTimelineLayersState,
} from '@vue/devtools-kit';

/** The events the backend broadcasts to the panel (devtools-core's `DevToolsMessagingEvents`). */
export const VUE_EVENTS = {
  INSPECTOR_TREE_UPDATED: 'inspector-tree-updated',
  INSPECTOR_STATE_UPDATED: 'inspector-state-updated',
  DEVTOOLS_STATE_UPDATED: 'devtools-state-updated',
  ROUTER_INFO_UPDATED: 'router-info-updated',
  TIMELINE_EVENT_UPDATED: 'timeline-event-updated',
  INSPECTOR_UPDATED: 'inspector-updated',
  ACTIVE_APP_UNMOUNTED: 'active-app-updated',
} as const;

type Payload = Record<string, unknown>;

/** The kit's state as devtools-core's `devtoolsState` answers it. */
export function devtoolsState(): Record<string, unknown> {
  const state = devtools.ctx.state;
  return {
    connected: state.connected,
    clientConnected: true,
    vueVersion: state.activeAppRecord?.version ?? '',
    tabs: state.tabs,
    commands: state.commands,
    vitePluginDetected: state.vitePluginDetected,
    appRecords: state.appRecords.map((item) => ({
      id: item.id,
      name: item.name,
      version: item.version,
      routerId: item.routerId,
      iframe: item.iframe,
    })),
    activeAppRecordId: state.activeAppRecordId,
    timelineLayersState: state.timelineLayersState,
  };
}

/** The server function table (devtools-core's names). */
export const functions = {
  heartbeat: (): boolean => true,
  devtoolsState,
  async getInspectorTree(payload: Payload): Promise<string | string[]> {
    return stringify(await devtools.ctx.api.getInspectorTree(payload as never));
  },
  async getInspectorState(payload: Payload): Promise<string | string[]> {
    const inspector = getInspector(String(payload.inspectorId));
    if (inspector) inspector.selectedNodeId = String(payload.nodeId);
    return stringify(await devtools.ctx.api.getInspectorState(payload as never));
  },
  editInspectorState: (payload: Payload) => devtools.ctx.api.editInspectorState(payload as never),
  sendInspectorState: (id: string) => devtools.ctx.api.sendInspectorState(id),
  inspectComponentInspector: () => devtools.ctx.api.inspectComponentInspector(),
  cancelInspectComponentInspector: () => devtools.ctx.api.cancelInspectComponentInspector(),
  scrollToComponent: (id: string) => devtools.ctx.api.scrollToComponent(id),
  inspectDOM: (id: string) => devtools.ctx.api.inspectDOM(id),
  getInspectorNodeActions: (id: string) => getInspectorNodeActions(id),
  getInspectorActions: (id: string) => getInspectorActions(id),
  callInspectorNodeAction(inspectorId: string, actionIndex: number, nodeId: string): void {
    const actions = getInspectorNodeActions(inspectorId);
    if (actions?.length) actions[actionIndex]?.action?.(nodeId);
  },
  callInspectorAction(inspectorId: string, actionIndex: number): void {
    const actions = getInspectorActions(inspectorId);
    if (actions?.length) actions[actionIndex]?.action?.();
  },
  toggleApp: (id: string, options?: Payload) => devtools.ctx.api.toggleApp(id, options as never),
  getRouterInfo: () => devtoolsRouterInfo,
  toggleClientConnected: (state: boolean) => toggleClientConnected(state),
  /** The timeline's switches (RB-080-31): `recordingState` and the built-in layers' flags, a
   *  plugin's layers under its plugin id; the kit keeps them in the page's localStorage. */
  updateTimelineLayersState: (state: Record<string, boolean>): null => {
    updateTimelineLayersState(state);
    return null;
  },
  getCustomInspector: () => getActiveInspectors(),
  getInspectorInfo: (id: string) => getInspectorInfo(id),
  highlightComponent: (uid: string) =>
    devtools.ctx.hooks.callHook(DevToolsContextHookKeys.COMPONENT_HIGHLIGHT, { uid }),
  unhighlight: () => devtools.ctx.hooks.callHook(DevToolsContextHookKeys.COMPONENT_UNHIGHLIGHT),
};

/** What the panel's RPC client exposes: `emit(event, payload)` takes the broadcasts. */
interface PanelFunctions {
  emit: (event: string, payload?: unknown) => void;
}

/** Broadcasts the kit's updates to every connected panel (devtools-core's
 *  `initDevToolsServerListener`); once, after the RPC server exists. */
export function broadcastUpdates(): void {
  const { broadcast } = getRpcServer<PanelFunctions>();
  const hooks = devtools.ctx.hooks;
  hooks.hook(DevToolsMessagingHookKeys.SEND_INSPECTOR_TREE_TO_CLIENT, (payload) => {
    broadcast.emit(VUE_EVENTS.INSPECTOR_TREE_UPDATED, stringify(payload));
  });
  hooks.hook(DevToolsMessagingHookKeys.SEND_INSPECTOR_STATE_TO_CLIENT, (payload) => {
    broadcast.emit(VUE_EVENTS.INSPECTOR_STATE_UPDATED, stringify(payload));
  });
  hooks.hook(DevToolsMessagingHookKeys.DEVTOOLS_STATE_UPDATED, () => {
    broadcast.emit(VUE_EVENTS.DEVTOOLS_STATE_UPDATED, devtoolsState());
  });
  hooks.hook(DevToolsMessagingHookKeys.ROUTER_INFO_UPDATED, ({ state }) => {
    broadcast.emit(VUE_EVENTS.ROUTER_INFO_UPDATED, state);
  });
  hooks.hook(DevToolsMessagingHookKeys.SEND_INSPECTOR_TO_CLIENT, (payload) => {
    broadcast.emit(VUE_EVENTS.INSPECTOR_UPDATED, payload);
  });
  hooks.hook(DevToolsMessagingHookKeys.SEND_TIMELINE_EVENT_TO_CLIENT, (payload) => {
    broadcast.emit(VUE_EVENTS.TIMELINE_EVENT_UPDATED, stringify(payload));
  });
  hooks.hook(DevToolsMessagingHookKeys.SEND_ACTIVE_APP_UNMOUNTED_TO_CLIENT, () => {
    broadcast.emit(VUE_EVENTS.ACTIVE_APP_UNMOUNTED);
  });
}
