// The Endpoints tab's state (RB-096-06): the answer of `endpoints` asked with the tab's filters
// (one read at a time, the newest shown), the selected endpoint, the check that reports the
// findings, and the jump to an endpoint's requests in the Requests tab.
import { computed, reactive, ref, watch } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import { type EndpointFilters, type EndpointsAnswer, SORTS, endpointArgs } from './endpointModel';

export type EndpointsStore = ReturnType<typeof createEndpoints>;

export function createEndpoints(panel: Panel, showRequests: (key: string) => void) {
  const answer = ref<EndpointsAnswer | null>(null);
  const filters = reactive<EndpointFilters>({ search: '', sort: SORTS[0] ?? 'p95', currentPage: false });
  const selected = ref<string | null>(null);
  const note = ref('');

  const reader = latestRefresh(
    () => panel.call({ tool: 'endpoints' }, endpointArgs(filters)),
    (raw) => {
      const value = raw as EndpointsAnswer;
      if (value && Array.isArray(value.endpoints) && Array.isArray(value.views)) answer.value = value;
    },
    (error) => {
      note.value = panel.error(error);
    },
  );

  const refresh = (): Promise<void> => (panel.ready.value ? reader.refresh() : Promise.resolve());
  const poll = (): Promise<void> => (panel.ready.value ? reader.poll() : Promise.resolve());

  watch(
    () => ({ ...filters }),
    () => void refresh(),
  );

  const endpoint = computed(() => (answer.value?.endpoints ?? []).find((one) => one.key === selected.value));

  /** The label of view `id` as the answer names it. */
  function label(id: number): string {
    return answer.value?.views.find((view) => view.id === id)?.label ?? String(id);
  }

  /** The findings of the current page about `key`. */
  function findingsOf(key: string) {
    return (answer.value?.findings ?? []).filter((finding) => finding.endpoint === key);
  }

  function check(): void {
    panel.call({ command: 'endpoints_check' }).then(
      (value) => {
        note.value = panel.t('endpoints.checked', (value ?? {}) as Record<string, unknown>);
        void refresh();
      },
      (error: unknown) => {
        note.value = panel.error(error);
      },
    );
  }

  return {
    answer,
    filters,
    selected,
    endpoint,
    note,
    refresh,
    poll,
    label,
    findingsOf,
    check,
    showRequests,
  };
}
