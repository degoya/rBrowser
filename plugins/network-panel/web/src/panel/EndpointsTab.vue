<script setup lang="ts">
// The Endpoints tab (RB-096-06): the page's requests grouped by method and path with ids as
// placeholders, from what the views already loaded - nothing is sent. Filters (text, sort,
// current page only) and Check, which reports the rules' findings into the findings panel; per
// endpoint its calls, median and p95, waiting time, size and status mix, the issues it has on
// the current page as a badge beside its name; the chosen one's detail beside the list (below it
// when narrow).
import { computed } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import { type Endpoint, SORTS, statusMix } from './endpointModel';
import EndpointDetail from './EndpointDetail.vue';
import { duration, size } from './model';
import { useNetwork } from './store';

const net = useNetwork();
const ep = net.endpoints;
const t = net.panel.t;

const sortItems = computed(() => SORTS.map((sort) => ({ label: t('endpoints.sorts.' + sort), value: sort })));
const rows = computed(() => ep.answer.value?.endpoints ?? []);

const right = { th: 'text-right', td: 'text-right tabular-nums' };
const wideRight = { th: 'hidden sm:table-cell text-right', td: 'hidden sm:table-cell text-right tabular-nums' };
const columns = computed<TableColumn<Endpoint>[]>(() => [
  { id: 'endpoint', header: t('endpoints.columns.endpoint'), meta: { class: { td: 'max-w-0 w-full whitespace-normal' } } },
  { id: 'calls', header: t('endpoints.columns.calls'), meta: { class: right } },
  { id: 'median', header: t('endpoints.columns.median'), meta: { class: wideRight } },
  { id: 'p95', header: t('endpoints.columns.p95'), meta: { class: right } },
  { id: 'ttfb', header: t('endpoints.columns.ttfb'), meta: { class: wideRight } },
  { id: 'size', header: t('endpoints.columns.size'), meta: { class: wideRight } },
  { id: 'status', header: t('endpoints.columns.status'), meta: { class: { th: 'hidden md:table-cell', td: 'hidden md:table-cell' } } },
]);

const meta = computed(() => ({
  class: { tr: (row: { original: Endpoint }) => (row.original.key === ep.selected.value ? 'bg-primary/10' : '') },
}));

/** What the list cannot show, each once. */
const notes = computed(() => {
  const answer = ep.answer.value;
  if (!answer) return [];
  const out: { key: string; tone: 'warning' | 'info'; text: string }[] = [];
  if (!answer.captured) out.push({ key: 'capture', tone: 'warning', text: t('status.noCapture') });
  if (answer.total > answer.endpoints.length) out.push({ key: 'truncated', tone: 'info', text: t('endpoints.truncated', { shown: answer.endpoints.length, total: answer.total }) });
  return out;
});
</script>

<template>
  <div id="panel-endpoints" class="flex flex-col gap-2 md:min-h-0 md:flex-1">
    <div class="flex flex-wrap items-center gap-2">
      <UInput
        id="endpoint-search"
        v-model="ep.filters.search"
        type="search"
        icon="i-lucide-search"
        size="sm"
        :maxlength="200"
        autocomplete="off"
        :spellcheck="false"
        :placeholder="t('endpoints.search')"
        :aria-label="t('endpoints.search')"
        class="min-w-40 flex-1"
      />
      <USelect id="endpoint-sort" v-model="ep.filters.sort" :items="sortItems" size="sm" :aria-label="t('endpoints.sort')" class="w-44" />
      <UCheckbox id="endpoint-current" v-model="ep.filters.currentPage" size="sm" :label="t('endpoints.currentPage')" />
      <UButton id="endpoint-check" size="sm" icon="i-lucide-list-checks" :label="t('endpoints.check')" @click="ep.check()" />
    </div>
    <p v-if="ep.note.value" id="endpoints-note" role="status" class="text-sm text-muted">{{ ep.note.value }}</p>
    <div v-if="notes.length" id="endpoint-notes" class="flex flex-col gap-1.5">
      <UAlert
        v-for="note in notes"
        :key="note.key"
        :data-note="note.key"
        :color="note.tone"
        variant="subtle"
        :icon="note.tone === 'warning' ? 'i-lucide-triangle-alert' : 'i-lucide-info'"
        :title="note.text"
        :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
      />
    </div>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-3 md:min-h-0 md:flex-1 md:grid-rows-[minmax(0,1fr)]" :class="ep.endpoint.value ? 'md:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]' : undefined">
      <section aria-labelledby="endpoints-title" class="flex min-h-64 flex-col md:min-h-0">
        <h2 id="endpoints-title" class="sr-only">{{ t('endpoints.title') }}</h2>
        <UEmpty v-if="rows.length === 0" id="endpoints-empty" icon="i-lucide-route" :title="t('endpoints.empty')" variant="naked" />
        <UTable
          v-else
          id="endpoints"
          :data="rows"
          :columns="columns"
          :meta="meta"
          :get-row-id="(row: Endpoint) => row.key"
          sticky
          class="max-h-[60dvh] min-h-0 flex-1 rounded-md border border-default md:max-h-none"
          :ui="{ th: 'px-2 py-1.5 whitespace-nowrap', td: 'px-2 py-1.5 whitespace-nowrap text-sm' }"
        >
          <template #endpoint-cell="{ row }">
            <div class="flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-0.5">
              <span class="font-mono text-xs text-muted">{{ row.original.method }}</span>
              <UButton
                variant="link"
                color="neutral"
                size="sm"
                :data-endpoint="row.original.key"
                :aria-current="row.original.key === ep.selected.value ? 'true' : undefined"
                :title="row.original.origin + row.original.path"
                :label="row.original.path"
                :ui="{ base: 'min-w-0 p-0 text-highlighted', label: 'truncate' }"
                @click="ep.selected.value = row.original.key"
              />
              <UBadge v-if="row.original.issues.length" data-issues color="warning" variant="subtle" size="sm" class="shrink-0" :label="t('endpoints.issues', { count: row.original.issues.length })" />
            </div>
          </template>
          <template #calls-cell="{ row }">{{ row.original.calls }}</template>
          <template #median-cell="{ row }">{{ duration(row.original.durationMs?.median) }}</template>
          <template #p95-cell="{ row }"><span data-p95>{{ duration(row.original.durationMs?.p95) }}</span></template>
          <template #ttfb-cell="{ row }">{{ duration(row.original.ttfbMs?.p95) }}</template>
          <template #size-cell="{ row }">{{ size(row.original.decoded?.median) }}</template>
          <template #status-cell="{ row }">{{ statusMix(row.original.statuses, t('endpoints.failed')) }}</template>
        </UTable>
      </section>
      <EndpointDetail v-if="ep.endpoint.value" />
    </div>
  </div>
</template>
