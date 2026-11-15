<script setup lang="ts">
// One cell of the storage grid (RB-070-01, on Nuxt UI since RB-080-84): the view's value - JSON
// coloured, `…` when cut, HttpOnly as a badge, a cell that differs from the views' majority tinted
// and marked with an icon and in words -, `missing` where the entry applies but is absent, a dash
// where the view shows another page, `not read` for a view the panel cannot read (the grid
// tints the table cell). It is a grid
// cell with one tab stop in the grid (`focused`); the native title names a cookie's attributes
// (a tooltip component per cell would weigh on a grid of hundreds of cells).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import ValueText from './ValueText.vue';
import { type StorageRow, type StorageView, cellState, expiresText } from './model';

const props = defineProps<{ row: StorageRow; view: StorageView; index: number; col: number; focused: boolean; differs: boolean }>();
const { t } = usePanel();

const state = computed(() => cellState(props.row, props.view));
const data = computed(() => props.row.cells[String(props.view.id)] ?? null);
const title = computed(() => {
  const cell = data.value;
  if (state.value !== 'value' || !cell || typeof cell.domain !== 'string') return undefined;
  return [cell.domain + (cell.path ?? ''), expiresText(cell.expires, t('panel.sessionCookie')), cell.secure ? t('panel.secure') : '', cell.sameSite ?? '']
    .filter(Boolean)
    .join(' · ');
});
</script>

<template>
  <div
    data-cell
    role="gridcell"
    :data-row="index"
    :data-col="col"
    :data-view="view.id"
    :data-state="state"
    :data-differs="differs && state === 'value' ? '' : undefined"
    :tabindex="focused ? 0 : -1"
    :title="title"
    :aria-label="state === 'na' ? t('panel.notApplicable') : undefined"
    class="-mx-2 -my-1.5 flex min-h-8 max-w-64 cursor-pointer items-center gap-1 px-2 py-1.5 outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
  >
    <template v-if="state === 'value'">
      <UIcon v-if="differs" name="i-lucide-git-compare-arrows" class="size-3.5 shrink-0" aria-hidden="true" />
      <ValueText :text="data?.value ?? ''" />
      <span v-if="data?.truncated" data-more class="shrink-0 text-muted">…</span>
      <UBadge v-if="data?.httpOnly" data-flag color="neutral" variant="outline" size="sm" class="shrink-0" :label="t('panel.httpOnly')" />
      <span v-if="differs" class="sr-only"> ({{ t('panel.state.differs') }})</span>
    </template>
    <span v-else-if="state === 'missing'" data-mark class="text-xs">{{ t('panel.missing') }}</span>
    <span v-else-if="state === 'na'" data-mark aria-hidden="true">–</span>
    <span v-else data-mark class="text-xs">{{ t('panel.unsupported') }}</span>
  </div>
</template>
