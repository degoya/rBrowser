<script setup lang="ts">
// A small picture of the grid beside its fields (Settings > Layout > Grid, RB-080-13): columns x
// rows with each cell where `layout.cells` puts it, named by its breakpoint's label. Read-only;
// the cells are edited in the list beside it.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { JsonValue } from '../ipc/protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

const props = defineProps<{
  /** The value of the `layout` key. */
  layout: JsonValue;
  /** The label of each breakpoint id; an id without one shows as itself. */
  names: ReadonlyMap<string, string>;
}>();
const { t } = useI18n();

/** The preview stays a sketch: at most this many tracks either way. */
const MAX_TRACKS = 12;

function count(value: unknown, fallback = 1): number {
  return typeof value === 'number' && value >= 1 ? Math.min(Math.floor(value), MAX_TRACKS) : fallback;
}

const grid = computed(() => (isObject(props.layout) ? props.layout : {}));
const columns = computed(() => count(grid.value.columns));
const rows = computed(() => count(grid.value.rows));
const cells = computed(() =>
  (Array.isArray(grid.value.cells) ? grid.value.cells : []).flatMap((cell) => (isObject(cell) ? [cell] : [])).map((cell, index) => {
    const id = typeof cell.breakpoint === 'string' ? cell.breakpoint : '';
    return {
      key: `${index}-${id}`,
      name: props.names.get(id) ?? id,
      style: {
        gridColumn: `${count(cell.col)} / span ${count(cell.colSpan)}`,
        gridRow: `${count(cell.row)} / span ${count(cell.rowSpan)}`,
      },
    };
  }),
);
</script>

<template>
  <figure class="flex w-56 shrink-0 flex-col gap-1" data-testid="grid-preview">
    <figcaption class="text-xs font-medium text-muted">{{ t('settings.grid_preview.title') }}</figcaption>
    <div
      role="img"
      :aria-label="t('settings.grid_preview.label', { columns: String(columns), rows: String(rows), n: cells.length })"
      class="grid aspect-video gap-1 rounded-md bg-elevated p-1.5"
      :style="{
        gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
        gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))`,
      }"
    >
      <div
        v-for="cell in cells"
        :key="cell.key"
        class="flex min-w-0 items-center justify-center rounded-sm bg-primary/15 px-1 ring ring-primary/40"
        :style="cell.style"
        data-testid="grid-preview-cell"
      >
        <span class="truncate text-xs text-highlighted">{{ cell.name }}</span>
      </div>
    </div>
    <p v-if="cells.length === 0" class="text-xs text-muted">{{ t('settings.grid_preview.empty') }}</p>
  </figure>
</template>
