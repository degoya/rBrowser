// The page's activity store (RB-090-67): one per shell page (`bootstrap.ts` provides it), shared
// by the bar (`ActivityBar.vue`), the toolbar's compact forms (`ActivityIndicators.vue`,
// `RecordIndicator.vue`, `DesyncIndicator.vue`). It reads what the activities already publish -
// the snapshot, `getFlowRun` again on every `flowRunChanged`, the pushed `checksProgress`,
// `getPortal` again on every `portalChanged` - from its first use on, never by polling; a clock
// ticks only while a video runs, for its time. The toolbar's page alone (`App.vue`) also keeps
// the core's room for the bar in step (`manageRoom`).

import { computed, inject, onScopeDispose, ref, shallowRef, watch, type InjectionKey } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { Activity, ActivityBarPosition } from '../ipc/activity';
import type { ChecksProgress, CoreToShell, FlowRunState, PortalState } from '../ipc/protocol';
import { loadPortal } from '../portal/portal';
import { useShell, type Shell } from '../state/shell';
import type { Subscribe } from '../whichkey/whichkey';
import { barEntries, barSettings, drawnAt, ended, running, wantedPlace, withStep, type ActivityInputs } from './activities';
import type { EntryData } from './entries';

/** Where the store reads the activities that are not in the snapshot. */
export interface ActivitySource {
  flowRun: () => Promise<FlowRunState | null>;
  checks: () => Promise<ChecksProgress | null>;
  portal: () => Promise<PortalState | null>;
  subscribe: Subscribe;
}

/** The source over a page's IPC client; a request the core refuses reads as nothing running. */
export function activitySource(client: IpcClient): ActivitySource {
  return {
    flowRun: async () => {
      const { type: _type, ...state } = await client.expect({ type: 'getFlowRun' }, 'flowRun');
      return state;
    },
    checks: async () => {
      const answer = await client.request({ type: 'getChecksProgress' });
      return answer.type === 'checksProgress' ? withoutType(answer) : null;
    },
    portal: () => loadPortal(client),
    subscribe: (listener) => client.on(listener),
  };
}

function withoutType(message: Extract<CoreToShell, { type: 'checksProgress' }>): ChecksProgress {
  const { type: _type, ...progress } = message;
  return progress;
}

export type Activities = ReturnType<typeof createActivities>;

export const ACTIVITIES_KEY: InjectionKey<Activities> = Symbol('rb-activities');

/** The page's store; without one provided (a test), a store of the snapshot alone. */
export function useActivities(): Activities {
  const store = inject(ACTIVITIES_KEY, null) ?? createActivities(useShell(), null);
  store.start();
  return store;
}

export function createActivities(shell: Shell, source: ActivitySource | null) {
  const flowRun = shallowRef<FlowRunState | null>(null);
  const checks = shallowRef<ChecksProgress | null>(null);
  const portal = shallowRef<PortalState | null>(null);
  const now = ref(Date.now());
  let started = false;

  const read = <T>(load: () => Promise<T | null>, into: { value: T | null }): void => {
    load()
      .then((value) => (into.value = value))
      .catch(() => (into.value = null));
  };

  /** Reads the activities and follows the core's events; once per page. */
  function start(): void {
    if (started || source === null) {
      return;
    }
    started = true;
    read(source.flowRun, flowRun);
    read(source.checks, checks);
    read(source.portal, portal);
    source.subscribe((message) => {
      if (message.type === 'flowRunChanged') read(source.flowRun, flowRun);
      // The step each view is at (RB-090-68), between the reads.
      if (message.type === 'flowStep') flowRun.value = withStep(flowRun.value, message);
      if (message.type === 'portalChanged') read(source.portal, portal);
      if (message.type === 'checksProgress') checks.value = withoutType(message);
    });
  }

  // The video's time: a tick a second while one runs, none otherwise.
  let timer: ReturnType<typeof setInterval> | null = null;
  watch(
    () => shell.state.value?.video !== undefined,
    (video) => {
      if (timer !== null) clearInterval(timer);
      timer = video ? setInterval(() => (now.value = Date.now()), 1000) : null;
      now.value = Date.now();
    },
    { immediate: true },
  );

  const inputs = computed<ActivityInputs>(() => ({
    state: shell.state.value,
    flowRun: flowRun.value,
    checks: checks.value,
    portal: portal.value,
  }));
  const settings = computed(() => barSettings(shell.settings.value));
  const run = computed(() => running(inputs.value));
  const collapsed = computed<Activity[]>(() => shell.state.value?.activityBar?.collapsed ?? []);
  const entries = computed(() => barEntries(run.value, settings.value, collapsed.value));
  const granted = computed(() => shell.state.value?.activityBar?.place);

  /** The entries the page at `place` draws now. */
  function drawn(place: ActivityBarPosition): Activity[] {
    return drawnAt(place, granted.value, entries.value);
  }

  /** In the bar now: its toolbar indicator steps back. */
  function inBar(activity: Activity): boolean {
    return granted.value !== undefined && entries.value.includes(activity);
  }

  /** Folded by its close button while it runs, and the bar would take it back. */
  function folded(activity: Activity): boolean {
    const { mode, show } = settings.value;
    return mode !== 'indicator' && show[activity] && collapsed.value.includes(activity) && run.value.includes(activity);
  }

  /** Folds `activity` into its toolbar indicator, or shows it in the bar again. */
  function collapse(activity: Activity, fold: boolean): void {
    void shell.send({ type: 'collapseActivity', activity, collapsed: fold });
  }

  /** What `entryView` draws from. */
  function data(): EntryData {
    const views = shell.state.value?.views ?? [];
    return {
      inputs: inputs.value,
      nowMs: now.value,
      viewLabel: (id) => views.find((view) => view.id === id)?.label ?? String(id),
    };
  }

  /**
   * The toolbar's page only: asks the core for the bar's room while it has entries - always with
   * `ui.activityBar.mode` `always` - gives it back after the last, and opens a folded entry again once its activity ended.
   */
  function manageRoom(): void {
    let asked: ActivityBarPosition | null | undefined;
    watch(
      () => wantedPlace(entries.value, settings.value),
      (place) => {
        if (place === asked) return;
        asked = place;
        void shell.ask(place === null ? { type: 'activityBar' } : { type: 'activityBar', place });
      },
      { immediate: true },
    );
    watch(
      () => ended(collapsed.value, run.value).join(' '),
      (list) => {
        for (const activity of list.split(' ').filter(Boolean) as Activity[]) {
          void shell.ask({ type: 'collapseActivity', activity, collapsed: false });
        }
      },
      { immediate: true },
    );
  }

  onScopeDispose(() => {
    if (timer !== null) clearInterval(timer);
  }, true);

  return { inputs, settings, run, entries, granted, now, start, drawn, inBar, folded, collapse, data, manageRoom };
}
