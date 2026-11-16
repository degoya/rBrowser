// The network panel's state (RB-070-02, on Nuxt UI since RB-080-84): the live list (`list`, asked
// with the filters), a saved session shown instead of it, the selected request, the tab, the
// status line and the site-data confirmation, with the debugger's, the mocks' and the sessions'
// parts. Provided to every component of the panel (`useNetwork()`).
import { computed, inject, reactive, ref, type InjectionKey } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import { createDebug } from './debug';
import { createMocks } from './mocks';
import { type Filters, keyOf, listArgs } from './model';
import { createSessions } from './sessions';
import type { ListAnswer, ListRow, SessionAnswer } from './types';

export type Tab = 'requests' | 'breakpoints' | 'compose' | 'sessions' | 'mocks';
export type NetworkStore = ReturnType<typeof createNetwork>;
export const NETWORK_KEY: InjectionKey<NetworkStore> = Symbol('network');

/** How many list refreshes an outcome stays in the status line. */
const OUTCOME_POLLS = 2;

export function useNetwork(): NetworkStore {
  const store = inject(NETWORK_KEY);
  if (!store) throw new Error('useNetwork() outside the network panel');
  return store;
}

export function createNetwork(panel: Panel) {
  const answer = ref<ListAnswer | null>(null);
  const session = ref<SessionAnswer | null>(null);
  const filters = reactive<Filters>({ search: '', type: '', status: 'all', differences: false });
  const selected = ref<string | null>(null);
  const tab = ref<Tab>('requests');
  const confirming = ref(false);
  /** An outcome or failure for the status line, and how many refreshes it stays. */
  const outcome = ref<(StatusLine & { polls: number }) | null>(null);

  /** The rows and views shown: the open session's, else the live list's. */
  const shown = computed(() => session.value ?? answer.value);
  const views = () => shown.value?.views ?? [];

  const debug = createDebug(panel, () => answer.value?.views ?? []);
  const mocks = createMocks(panel, () => void refresh());
  const sessions = createSessions(panel, (opened) => {
    session.value = opened;
    selected.value = null;
    if (opened) tab.value = 'requests';
  });

  function say(tone: StatusLine['tone'], text: string): void {
    outcome.value = { tone, text, polls: OUTCOME_POLLS };
  }

  /** The status line: an outcome or failure, else the open session. */
  const status = computed<StatusLine | null>(() => {
    if (outcome.value) return outcome.value;
    if (session.value) return { tone: 'info', text: panel.t('sessions.viewing', { path: session.value.path }) };
    return null;
  });

  // One read at a time; a filter, an event or an action during a read reads again right after
  // it, and the overtaken answer is not shown (review of RB-080-84).
  const reader = latestRefresh(
    () => panel.call({ tool: 'list' }, listArgs(filters)),
    (raw) => {
      const value = raw as ListAnswer;
      // Only an answer of the list's shape replaces the list.
      if (value && Array.isArray(value.rows) && Array.isArray(value.views)) answer.value = value;
      if (outcome.value && outcome.value.tone !== 'error' && --outcome.value.polls < 0) outcome.value = null;
    },
    (error) => say('error', panel.error(error)),
  );

  function refresh(): Promise<void> {
    return panel.ready.value && !session.value ? reader.refresh() : Promise.resolve();
  }

  /** The timer's read: a read on its way stands. */
  function poll(): Promise<void> {
    return panel.ready.value && !session.value ? reader.poll() : Promise.resolve();
  }

  /** Runs `what`, then says the outcome in the status line and refreshes. */
  function act(what: { command: string } | { tool: string }, args: unknown, done: string): void {
    panel.call(what as { command: string }, args).then(
      (value) => {
        say('ok', panel.t(done, (value ?? {}) as Record<string, unknown>));
        void refresh();
      },
      (error: unknown) => say('error', panel.error(error)),
    );
  }

  const selectedRow = computed<ListRow | undefined>(() => (shown.value?.rows ?? []).find((row) => keyOf(row) === selected.value));

  /** The newest instance of the selected row in its first view, for a repeat. */
  function selectedSeen(): { view: number; id: string } | null {
    const row = (answer.value?.rows ?? []).find((candidate) => keyOf(candidate) === selected.value);
    const [view, seen] = row ? (Object.entries(row.views)[0] ?? []) : [];
    return view && seen ? { view: Number(view), id: seen.id } : null;
  }

  function repeat(times: number): void {
    const seen = selectedSeen();
    if (!seen) return;
    debug.repeat(seen.view, seen.id, times);
    tab.value = 'compose';
  }

  function mockThis(): void {
    const row = (answer.value?.rows ?? []).find((candidate) => keyOf(candidate) === selected.value);
    if (!row) return;
    const code = Object.values(row.views)
      .map((seen) => seen.status)
      .find((one) => typeof one === 'number');
    mocks.openEditor({ url: row.url, method: row.method, status: code });
    tab.value = 'mocks';
  }

  return {
    panel,
    answer,
    session,
    shown,
    views,
    filters,
    selected,
    selectedRow,
    tab,
    confirming,
    status,
    debug,
    mocks,
    sessions,
    say,
    refresh,
    poll,
    act,
    repeat,
    mockThis,
    clearSite(): void {
      confirming.value = false;
      act({ command: 'site_data_clear' }, { confirmed: true }, 'done.cleared');
    },
  };
}
