<script setup lang="ts">
// The requests (RB-070-02, on Nuxt UI since RB-080-84): a `UTable` that draws only the rows in
// sight (`virtualize`, TanStack Virtual - up to the plugin's limit of rows stay fluid), one row per
// method and URL with its status, type, size, time, a dot per view and the marks `partial` and
// `differs` in words. The keyboard is a list's: one tab stop (the focused row's name), arrows,
// Home and End move, Enter or Space opens; a click opens too.
import { computed, h, nextTick, ref } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import { duration, keyOf, moveFocus, shortUrl, size, statusOf } from './model';
import type { ListRow } from './types';
import { useNetwork } from './store';
import ViewDots from './ViewDots.vue';

/** A row's height in pixels, the virtualizer's first guess before it measures. */
const ROW_PX = 33;

const net = useNetwork();
const t = net.panel.t;
const focus = ref(0);
const table = ref<{ $el: HTMLElement } | null>(null);

const rows = computed(() => net.shown.value?.rows ?? []);
const views = computed(() => net.shown.value?.views ?? []);
const at = computed(() => Math.min(focus.value, Math.max(rows.value.length - 1, 0)));

const right = { th: 'text-right', td: 'text-right tabular-nums' };
/** Left out below the small breakpoint (the detail names them per view). */
const wide = { th: 'hidden sm:table-cell', td: 'hidden sm:table-cell' };
const wideRight = { th: 'hidden sm:table-cell text-right', td: 'hidden sm:table-cell text-right tabular-nums' };
const columns = computed<TableColumn<ListRow>[]>(() => [
  { id: 'method', header: t('list.columns.method'), meta: { class: { td: 'font-mono text-xs' } } },
  { id: 'name', header: t('list.columns.name'), meta: { class: { td: 'max-w-0 w-full whitespace-normal' } } },
  { id: 'status', header: t('detail.status'), meta: { class: right } },
  { id: 'type', header: t('toolbar.type'), meta: { class: wide } },
  { id: 'size', header: t('detail.size'), meta: { class: right } },
  { id: 'time', header: t('detail.time'), meta: { class: wideRight } },
  { id: 'views', header: () => h('span', { class: 'sr-only' }, t('list.columns.views')) },
]);

/** The status cell's words: the codes, or blocked / failed. */
function statusText(row: ListRow): string {
  const status = statusOf(row);
  return status === 'blocked' || status === 'failed' ? t('list.' + status) : status;
}

const first = (row: ListRow) => Object.values(row.views)[0];

/** The row's tint: the selected one, else partial or differs (the badge says which in words). */
const meta = computed(() => ({
  class: {
    tr: (row: { original: ListRow }) =>
      keyOf(row.original) === net.selected.value ? 'bg-primary/10' : row.original.partial ? 'bg-warning/10' : row.original.differs ? 'bg-info/10' : '',
  },
}));

function open(index: number): void {
  const row = rows.value[index];
  if (!row) return;
  focus.value = index;
  net.selected.value = keyOf(row);
}

/** The name button of row `index`, scrolled into the drawn range first when it is not drawn. */
async function focusRow(index: number): Promise<void> {
  focus.value = index;
  const root = table.value?.$el;
  const find = () => root?.querySelector<HTMLElement>(`[data-row="${index}"]`);
  if (!find() && root) {
    root.scrollTop = Math.max(0, index * ROW_PX - root.clientHeight / 2);
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  }
  await nextTick();
  find()?.focus();
}

function onKey(event: KeyboardEvent): void {
  const button = (event.target as HTMLElement).closest<HTMLElement>('[data-row]');
  if (!button) return;
  const index = Number(button.dataset.row);
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    open(index);
    return;
  }
  const moved = moveFocus(index, event.key, rows.value.length);
  if (moved === null) return;
  event.preventDefault();
  void focusRow(moved);
}

function onClick(event: MouseEvent): void {
  const tr = (event.target as HTMLElement).closest('tr');
  const button = tr?.querySelector<HTMLElement>('[data-row]');
  if (button) open(Number(button.dataset.row));
}
</script>

<template>
  <section aria-labelledby="rows-title" class="flex min-h-64 flex-col md:min-h-0" @keydown="onKey" @click="onClick">
    <h2 id="rows-title" class="sr-only">{{ t('list.title') }}</h2>
    <UEmpty v-if="rows.length === 0" id="empty" icon="i-lucide-network" :title="t('list.empty')" variant="naked" />
    <UTable
      v-else
      id="rows"
      ref="table"
      :data="rows"
      :columns="columns"
      :meta="meta"
      :get-row-id="(row: ListRow) => keyOf(row)"
      :virtualize="{ estimateSize: ROW_PX, overscan: 12 }"
      sticky
      class="max-h-[60dvh] min-h-0 flex-1 rounded-md border border-default md:max-h-none"
      :ui="{ th: 'px-2 py-1.5 whitespace-nowrap', td: 'px-2 py-1.5 whitespace-nowrap text-sm', tr: 'cursor-pointer' }"
    >
      <template #method-cell="{ row }">{{ row.original.method }}</template>
      <template #name-cell="{ row }">
        <div class="flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-0.5">
          <button
            type="button"
            :data-row="row.index"
            :data-key="keyOf(row.original)"
            :tabindex="row.index === at ? 0 : -1"
            :aria-current="keyOf(row.original) === net.selected.value ? 'true' : undefined"
            :title="row.original.url"
            class="max-w-full min-w-12 truncate rounded-sm text-left text-highlighted outline-none focus-visible:ring-2 focus-visible:ring-primary"
            @focus="focus = row.index"
          >
            {{ shortUrl(row.original.url) }}
          </button>
          <UBadge v-if="row.original.partial" data-mark="partial" color="warning" variant="subtle" size="sm" class="shrink-0" :label="t('list.partial')" />
          <UBadge v-if="row.original.differs" data-mark="differs" color="info" variant="subtle" size="sm" class="shrink-0" :label="t('list.differs')" />
        </div>
      </template>
      <template #status-cell="{ row }">
        <span data-status :class="statusOf(row.original) === 'blocked' || statusOf(row.original) === 'failed' ? 'text-error' : ''">{{ statusText(row.original) }}</span>
      </template>
      <template #type-cell="{ row }">{{ row.original.type }}</template>
      <template #size-cell="{ row }">{{ size(first(row.original)?.size) }}</template>
      <template #time-cell="{ row }">{{ duration(first(row.original)?.durationMs) }}</template>
      <template #views-cell="{ row }">
        <ViewDots :row="row.original" :views="views" />
      </template>
    </UTable>
  </section>
</template>
