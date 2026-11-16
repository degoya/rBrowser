<script setup lang="ts">
// The plugin panels window (RB-060-12, `settings.html#/panels[/<plugin>.<panel>]`): every panel
// the loaded plugins offer, one entry each in the list, the chosen one in the content area - a
// manifest's `placement = "dialog"` too, which only keeps it from opening unasked (RB-080-59). A
// panel with unsaved work keeps its frame, hidden, while another is shown, and so does one that
// asked to be kept (`keep`, RB-110-16: the todo panel, slow to fill). A panel runs in
// `<iframe sandbox="allow-scripts">` - an opaque origin without `cefQuery`, no same-origin
// access, forms, popups, modals, downloads or top navigation; its files carry a CSP without
// network, and the window's own CSP lets the frame load `rbrowser://plugin` only. Its one
// permission is `allow="clipboard-write"` (a copy button, the screenshot editor's copy), never
// clipboard reading. It reaches its plugin through `bridge.ts` alone.
//
// Docked (RB-080-57) the page can be folded (RB-080-61, `dock.ts`) to the icon rail (RB-080-93):
// the side bar's icons alone (`PanelSideBar.vue`), a column beside the views, a row below them,
// the docked panels' state from the start. Panel frames opened before stay in the page, hidden,
// so folding loses nothing; a rail that never opened loads none - its first open shows the panel
// chosen in it, else Findings. The side bar shows icons with titles or icons alone.
//
// Above the plugins' panels stand the core's (`core.ts`): Findings (RB-080-69), the findings page
// itself in the content area - no frame, the page's own rights - kept once opened; Videos
// (RB-110-12) only while shown, so a hidden recording neither plays nor holds its files.
//
// The list (RB-080-97): the rail of a start asks while the core still loads its plugins, and the
// core refuses (`plugins.error.not_available`). The page says they load - a failed list is never
// "no panels" - and asks again whenever the core says the panels offered changed (the event
// `panelsChanged`: the plugins are there, a panel hidden, a project plugin under new trust); the
// panel shown, its frame and the rail stay.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import FindingsPage from '../findings/FindingsPage.vue';
import type { IpcClient } from '../ipc/client';
import LinkChooser from '../links/LinkChooser.vue';
import { useLinkOpener } from '../links/opener';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { PanelBridge, type IncomingMessage, type PanelTarget } from './bridge';
import { FINDINGS, VIDEOS, isCorePanel, keyOf, sideEntries } from './core';
import VideosPage from '../videos/VideosPage.vue';
import {
  DEFAULT_STATE,
  SIDEBAR_SETTING,
  TOGGLE_COMMAND,
  folded,
  iconsOnly,
  loadState,
  NARROW_PAGE,
  panelArgs,
  railSide,
  type DockState,
  type SidebarMode,
} from './dock';
import PanelDockBar from './PanelDockBar.vue';
import PanelSideBar from './PanelSideBar.vue';
import { listFailureOf, type ListFailure } from './listFailure';
import type { PanelInfo, PanelMessage, PanelRequest } from './protocol';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    client: IpcClient;
    routeKey?: string | null;
    subscribe?: Subscribe;
    messages?: Pick<Window, 'addEventListener' | 'removeEventListener'>;
    /** The page's width, for a side bar too narrow for titles; the window's when absent. */
    pageWidth?: number;
  }>(),
  { routeKey: null, subscribe: undefined, messages: undefined, pageWidth: undefined },
);
const { t, te } = useI18n();

const panels = ref<PanelInfo[]>([]);
/** A list came: the first panel is chosen, a `select` no longer waits. */
const loaded = ref(false);
/** Why the newest list failed, or `null` (`listFailure.ts`). */
const listFailure = ref<ListFailure | null>(null);
const selected = ref<string | null>(null);
/** The panel to open once the list is there: the route's, or a `select` that came first. */
let wanted: string | null = props.routeKey;

/** The panels holding unsaved work (their `dirty` message): their frames stay alive, hidden,
 * while another panel is shown, so the work is never lost by a switch (RB-080-59). */
const unsaved = ref<string[]>([]);
/** The panels that asked to be kept (their `keep` message): their frames stay alive, hidden,
 * until the panel leaves the list (RB-110-16). */
const keepAlive = ref<string[]>([]);
/** The frames in the page: the shown panel's and those with unsaved work or kept, in list order
 * (a frame moved in the page would load again). */
const kept = computed(() =>
  panels.value.filter((panel) => {
    const key = keyOf(panel);
    return key === selected.value || unsaved.value.includes(key) || keepAlive.value.includes(key);
  }),
);

async function ask(request: PanelRequest, options?: { timeoutMs: number }): Promise<PanelMessage> {
  const answer = await props.client.expect({ type: 'pluginPanel', request }, 'pluginPanel', options);
  return answer.message;
}

/** Each frame in the page, by panel, with its window and how to take it from the bridge. */
const frames = new Map<string, { element: HTMLIFrameElement; view: PanelTarget; detach: () => void }>();

function markUnsaved(target: PanelTarget, dirty: boolean): void {
  const key = [...frames.entries()].find(([, frame]) => frame.view === target)?.[0];
  if (key === undefined || unsaved.value.includes(key) === dirty) {
    return;
  }
  unsaved.value = dirty ? [...unsaved.value, key] : unsaved.value.filter((other) => other !== key);
}

function markKept(target: PanelTarget): void {
  const key = [...frames.entries()].find(([, frame]) => frame.view === target)?.[0];
  if (key !== undefined && !keepAlive.value.includes(key)) {
    keepAlive.value = [...keepAlive.value, key];
  }
}

// A panel's link opens through the core, asking first for `ui.openLinksIn: ask` (RB-080-92).
const links = useLinkOpener();
const bridge = new PanelBridge(ask, markUnsaved, { open: links.open }, markKept);

/** Takes the frame of panel `key` from the bridge (it left the page). */
function forget(key: string): void {
  frames.get(key)?.detach();
  frames.delete(key);
  if (unsaved.value.includes(key)) {
    unsaved.value = unsaved.value.filter((other) => other !== key);
  }
  if (keepAlive.value.includes(key)) {
    keepAlive.value = keepAlive.value.filter((other) => other !== key);
  }
}

/** Attaches `element` to the bridge; `false` while it has no window (not yet in the page). */
function attach(key: string, panel: PanelInfo, element: HTMLIFrameElement): boolean {
  const view = element.contentWindow;
  if (view === null) {
    return false;
  }
  frames.set(key, { element, view, detach: bridge.attach(panel.plugin, view) });
  return true;
}

/** Keeps the bridge to the frames in the page. Vue calls a function ref with the frame - before
 * a new parent is inserted, without a window yet, so it is attached after the render, still
 * before its document can post (RB-080-59) - and with `null` when the frame leaves the page. */
function frameRef(panel: PanelInfo) {
  const key = keyOf(panel);
  return (element: unknown) => {
    if (!(element instanceof HTMLIFrameElement)) {
      if (element === null) {
        forget(key);
      }
      return;
    }
    if (frames.get(key)?.element === element) {
      return;
    }
    forget(key);
    if (!attach(key, panel, element)) {
      void nextTick(() => {
        if (element.isConnected && !frames.has(key) && !attach(key, panel, element)) {
          console.warn(`plugin panel ${key}: its frame has no window, the bridge cannot reach it`);
        }
      });
    }
  };
}

function open(panel: PanelInfo): void {
  selected.value = keyOf(panel);
}

/** The side bar's entries: the core's panels, then the plugins'. */
const entries = computed(() => sideEntries(panels.value, t));
/** Findings, once shown, stay in the page (hidden behind another panel), as a frame with work. */
const findingsOpened = ref(false);
watch(selected, (key) => {
  if (key === FINDINGS) {
    findingsOpened.value = true;
  }
});

/** Where the panels are and how the side bar shows (`panels.*` settings). */
const dock = ref<DockState>(DEFAULT_STATE);
const windowWidth = ref(window.innerWidth);
const onResize = (): void => {
  windowWidth.value = window.innerWidth;
};
const pageWidth = computed(() => props.pageWidth ?? windowWidth.value);
const narrow = computed(() => pageWidth.value < NARROW_PAGE);
const sidebarIcons = computed(() => iconsOnly(dock.value, pageWidth.value));
const isFolded = computed(() => folded(dock.value));
const strip = computed(() => railSide(dock.value));

async function runCommand(id: string, args: Record<string, string | boolean>): Promise<void> {
  try {
    await props.client.request({ type: 'runCommand', id, args });
  } catch {
    // The core reports a failed command in the status bar.
  }
}

/** A panel a command opened while the dock is folded (a check's findings, `$open`): the fold
 * holds - only the person unfolds it - and the strip marks the entry (review point 2). */
const attention = ref<string | null>(null);
watch(isFolded, (now) => {
  if (!now) {
    attention.value = null;
    // A rail that never opened chose nothing: it opens at Findings (RB-080-79).
    if (railWaited && selected.value === null) {
      selected.value = FINDINGS;
    }
  }
});

/** Settings loads in order: an answer older than the newest load (or than a change the page
 * made itself) is dropped (review point 5). */
let loadSeq = 0;
function reloadDock(): Promise<void> {
  const seq = ++loadSeq;
  return loadState(props.client).then((state) => {
    if (seq === loadSeq) {
      dock.value = state;
    }
  });
}
/** The first load of the dock state, before the first panel is chosen; never fails. */
let dockLoaded: Promise<boolean> = Promise.resolve(true);
/** The page started as the rail and chose no panel: its first open chooses one. */
let railWaited = false;

/** An entry chosen in the side bar; in the folded icon rail it opens the dock too. */
function choose(key: string): void {
  attention.value = null;
  select(key);
  if (strip.value !== null) {
    void runCommand(TOGGLE_COMMAND, { collapsed: false, ...panelArgs(key) });
  }
}

/** The side bar's mode button: the setting, kept by the core; shown at once. */
function setSidebar(mode: SidebarMode): void {
  loadSeq += 1;
  dock.value = { ...dock.value, sidebar: mode };
  void props.client
    .request({ type: 'setSetting', key: SIDEBAR_SETTING, value: mode, scope: 'user' })
    .catch(() => undefined);
}

// The core keeps the panel the page shows, so a shortcut, MCP or the setting moves the panels
// with it (the existing `pluginPanel` channel; it opens and runs nothing).
watch(selected, (key) => {
  const panel = panelArgs(key);
  if (key !== null && isCorePanel(key)) {
    void props.client
      .request({ type: 'pluginPanel', request: { op: 'selectedCore', panel: key } })
      .catch(() => undefined);
  } else if (panel.plugin !== undefined && panel.panel !== undefined) {
    void props.client
      .request({ type: 'pluginPanel', request: { op: 'selected', plugin: panel.plugin, panel: panel.panel } })
      .catch(() => undefined);
  }
});

/** Opens the panel a command asked for; one the window does not offer is ignored. Before the
 * list is there the last request waits, core or plugin (review point 4); after a failed list a
 * core panel, which needs none, opens at once (RB-080-97). */
function select(key: string): void {
  if (!loaded.value && (listFailure.value === null || !isCorePanel(key))) {
    wanted = key;
    return;
  }
  if (isCorePanel(key)) {
    wanted = null;
    selected.value = key;
    return;
  }
  const panel = panels.value.find((candidate) => keyOf(candidate) === key);
  if (panel !== undefined) {
    open(panel);
  }
}

const onMessage = (event: Event): void => {
  void bridge.receive(event as unknown as IncomingMessage);
};
const target = props.messages ?? window;
const stopEvents = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'pluginPanel' && 'event' in message.message) {
    bridge.deliver(message.message.event);
  } else if (message.type === 'pluginPanel' && 'select' in message.message) {
    const asked = message.message.select;
    const key = 'core' in asked ? asked.core : `${asked.plugin}.${asked.panel}`;
    select(key);
    if (isFolded.value) {
      attention.value = key;
    }
  } else if (message.type === 'pluginPanel' && 'panelsChanged' in message.message) {
    void relist();
  } else if (message.type === 'settingsChanged') {
    void bridge.refreshSettings();
    if (message.keys.some((key) => key.startsWith('panels.'))) {
      reloadDock();
    }
  }
});

onMounted(async () => {
  target.addEventListener('message', onMessage);
  window.addEventListener('resize', onResize);
  dockLoaded = reloadDock().then(() => true);
  await relist();
});

/** List requests in order: an answer older than the newest request is dropped (RB-080-97). */
let listSeq = 0;
/** Asks for the list; a failure keeps the panels the page has and says why. */
async function loadList(): Promise<boolean> {
  const seq = ++listSeq;
  try {
    const answer = await ask({ op: 'list' });
    if (seq !== listSeq) {
      return false;
    }
    panels.value = 'panels' in answer ? answer.panels : [];
    listFailure.value = null;
    // A panel shown and no longer offered (hidden, its project plugin's trust revoked) gives way
    // to Findings, which is always there.
    const shown = selected.value;
    if (shown !== null && !isCorePanel(shown) && !panels.value.some((panel) => keyOf(panel) === shown)) {
      selected.value = FINDINGS;
    }
    return true;
  } catch (error) {
    if (seq === listSeq) {
      listFailure.value = listFailureOf(error, t, te);
    }
    return false;
  }
}

/** The list, at the start and whenever the core says it changed; the first chooses a panel. */
async function relist(): Promise<void> {
  if (await loadList()) {
    if (!loaded.value) {
      loaded.value = true;
      await chooseFirst();
    }
  } else if (!loaded.value && wanted !== null && isCorePanel(wanted)) {
    select(wanted);
  }
}

/** The panel the first list opens: the one asked for, else - nothing chosen yet - the first. */
async function chooseFirst(): Promise<void> {
  const key = wanted;
  wanted = null;
  if (key !== null && isCorePanel(key)) {
    selected.value = key;
    return;
  }
  const asked = panels.value.find((panel) => keyOf(panel) === key);
  if (asked !== undefined) {
    open(asked);
    return;
  }
  // A core panel chosen while the list was not there stays.
  if (selected.value !== null) {
    return;
  }
  // The rail of a start loads no panel before the person opens one (RB-080-93).
  if ((await dockLoaded) && isFolded.value) {
    railWaited = true;
    return;
  }
  // Unasked, the first panel that is not a dialog (`placement = "dialog"`: opened on demand only).
  const first = panels.value.find((panel) => panel.placement !== 'dialog') ?? panels.value[0];
  if (first !== undefined && selected.value === null) {
    open(first);
  }
}

onBeforeUnmount(() => {
  target.removeEventListener('message', onMessage);
  window.removeEventListener('resize', onResize);
  stopEvents();
  frames.forEach((frame) => frame.detach());
});
</script>

<template>
  <!-- RB-080-57: the header with where the panels sit, above the list and the panel; RB-080-61,
       RB-080-93: folded, only the icon rail shows - a column beside the views, a row below. -->
  <div class="flex h-screen flex-col bg-default text-default">
  <PanelDockBar v-if="strip === null" :client="client" :state="dock" :current="selected" />
  <main
    class="flex min-h-0 flex-1 bg-default text-default"
    :class="strip === 'bottom' ? 'flex-col' : ''"
    data-testid="panels-page"
  >
    <PanelSideBar
      :entries="entries"
      :selected="selected"
      :icons-only="sidebarIcons"
      :narrow="narrow"
      :strip="strip"
      :attention="attention"
      @open="choose"
      @sidebar="setSidebar"
      @expand="runCommand(TOGGLE_COMMAND, { collapsed: false })"
    />
    <section v-show="!isFolded" class="relative min-w-0 flex-1" data-testid="panels-content">
      <iframe
        v-for="panel in kept"
        v-show="keyOf(panel) === selected"
        :key="keyOf(panel)"
        :ref="frameRef(panel)"
        :src="panel.url"
        :title="panel.title"
        sandbox="allow-scripts"
        allow="clipboard-write"
        referrerpolicy="no-referrer"
        class="h-full w-full border-0"
        :data-testid="keyOf(panel) === selected ? 'panel-frame' : 'panel-frame-kept'"
      />
      <FindingsPage
        v-if="findingsOpened"
        v-show="selected === FINDINGS"
        :client="client"
        :subscribe="subscribe ?? windowEvents"
      />
      <VideosPage v-if="selected === VIDEOS" :client="client" :subscribe="subscribe ?? windowEvents" />
      <LinkChooser :opener="links" />
      <template v-if="kept.length === 0 && !isCorePanel(selected)">
        <p v-if="listFailure?.loading === true" class="p-4 text-sm text-muted" data-testid="panels-loading">
          {{ t('panels.loading') }}
        </p>
        <p v-else-if="listFailure !== null" class="p-4 text-sm text-muted" role="alert" data-testid="panels-failed">
          {{ listFailure.text }}
        </p>
        <p v-else-if="loaded" class="p-4 text-sm text-muted" data-testid="panels-empty">
          {{ t('panels.empty') }}
        </p>
      </template>
    </section>
  </main>
  </div>
</template>
