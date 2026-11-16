<script setup lang="ts">
// The storage grid (RB-070-01, on Nuxt UI since RB-080-84): a UTable with one row per cookie or
// key and one column per view, the views grouped under their session (TanStack column groups).
// One tab stop in the grid: the arrow keys, Home and End (Ctrl for the corners) move it, Enter
// or Space opens the cell (or a click), Delete removes a value in that view. Not virtualised: the
// plugin caps a list at its message budget (`truncated`, the status line says so) and the
// keyboard model needs every cell in the document. The name column stays in place when a narrow
// panel scrolls the views sideways.
import { computed, h, nextTick, ref } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import GridCell from './GridCell.vue';
import NameCell from './NameCell.vue';
import ViewHead from './ViewHead.vue';
import { type StorageList, type StorageRow, type StorageView, arrange, cellState, differing, moveCell } from './model';

const props = defineProps<{ list: StorageList | null }>();
const emit = defineEmits<{ open: [row: StorageRow, view: StorageView]; remove: [row: StorageRow, view: StorageView] }>();
const { t } = usePanel();

const focus = ref<[number, number]>([0, 0]);
const box = ref<HTMLElement | null>(null);
const rows = computed(() => props.list?.rows ?? []);
const arranged = computed(() => arrange(props.list?.views ?? []));
const differs = computed(() => new Map(rows.value.map((row) => [row.id, differing(row)])));

const TINT: Record<string, string> = { missing: 'bg-error/10 text-error', unsupported: 'bg-elevated text-muted', error: 'bg-elevated text-muted', na: 'text-muted' };

/** The tint of a cell's whole box: differing in the warning's colour, missing in the error's,
 * unread greyed (each also said in words inside the cell). */
function tint(row: StorageRow, view: StorageView): string {
  const state = cellState(row, view);
  if (state === 'value') return differs.value.get(row.id)?.has(view.id) ? 'bg-warning/10 text-warning' : '';
  return TINT[state] ?? '';
}

const at = (row: number, col: number): boolean => focus.value[0] === row && focus.value[1] === col;

const columns = computed<TableColumn<StorageRow>[]>(() => {
  const { columns: views, groups } = arranged.value;
  let start = 0;
  const grouped = groups.map((group, index) => {
    const members = views.slice(start, start + group.span);
    const first = start;
    start += group.span;
    return {
      id: `session-${index}`,
      header: () => h('span', { 'data-session': '', class: 'text-xs font-semibold text-muted' }, t('panel.session', { name: group.session })),
      columns: members.map((view, offset) => ({
        id: `view-${view.id}`,
        header: () => h(ViewHead, { view }),
        meta: { class: { td: (cell: { row: { original: StorageRow } }) => tint(cell.row.original, view) } },
        cell: ({ row }: { row: { original: StorageRow; index: number } }) =>
          h(GridCell, {
            row: row.original,
            view,
            index: row.index,
            col: first + offset + 1,
            focused: at(row.index, first + offset + 1),
            differs: differs.value.get(row.original.id)?.has(view.id) ?? false,
          }),
      })),
    };
  });
  return [
    {
      id: 'name',
      header: t('panel.keyHeader'),
      cell: ({ row }: { row: { original: StorageRow; index: number } }) => h(NameCell, { row: row.original, index: row.index, focused: at(row.index, 0) }),
    },
    ...grouped,
  ];
});

/** The row and view of a cell element. */
function targetOf(cell: HTMLElement): { row: StorageRow | undefined; view: StorageView | undefined } {
  const row = rows.value[Number(cell.dataset.row)];
  const views = arranged.value.columns;
  const view = views.find((one) => String(one.id) === cell.dataset.view) ?? views.find((one) => row?.cells[String(one.id)]);
  return { row, view };
}

function activate(cell: HTMLElement): void {
  const { row, view } = targetOf(cell);
  if (!row || !view) return;
  if (cell.dataset.state && !['value', 'missing'].includes(cell.dataset.state)) return;
  emit('open', row, view);
}

function cellAt(row: number, col: number): HTMLElement | null {
  return box.value?.querySelector<HTMLElement>(`[data-cell][data-row="${row}"][data-col="${col}"]`) ?? null;
}

function onKey(event: KeyboardEvent): void {
  const cell = (event.target as HTMLElement).closest<HTMLElement>('[data-cell]');
  if (!cell) return;
  const here: [number, number] = [Number(cell.dataset.row), Number(cell.dataset.col)];
  const moved = moveCell(here[0], here[1], event.key, rows.value.length, arranged.value.columns.length + 1, event.ctrlKey);
  if (moved) {
    event.preventDefault();
    focus.value = moved;
    void nextTick(() => cellAt(moved[0], moved[1])?.focus());
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    activate(cell);
  } else if (event.key === 'Delete' && cell.dataset.state === 'value') {
    event.preventDefault();
    const { row, view } = targetOf(cell);
    if (row && view && cellState(row, view) === 'value') emit('remove', row, view);
  }
}

function onClick(event: MouseEvent): void {
  const cell = (event.target as HTMLElement).closest<HTMLElement>('[data-cell]');
  if (!cell) return;
  focus.value = [Number(cell.dataset.row), Number(cell.dataset.col)];
  activate(cell);
}

/** Keeps the tab stop inside the grid when the list shrinks; puts the focus back on it. */
function clamp(): void {
  focus.value = [Math.min(focus.value[0], Math.max(rows.value.length - 1, 0)), Math.min(focus.value[1], arranged.value.columns.length)];
}

function restore(): void {
  void nextTick(() => cellAt(focus.value[0], focus.value[1])?.focus());
}

function reset(): void {
  focus.value = [0, 0];
}

defineExpose({ clamp, restore, reset, focused: () => box.value?.contains(document.activeElement) ?? false });
</script>

<template>
  <div ref="box" id="grid" @keydown="onKey" @click="onClick">
    <UTable
      :data="rows"
      :columns="columns"
      :get-row-id="(row: StorageRow) => row.id"
      :column-pinning="{ left: ['name'], right: [] }"
      :caption="t('panel.grid')"
      :empty="t('panel.empty')"
      sticky
      :ui="{ caption: 'sr-only', th: 'px-2 py-1.5 align-bottom text-left', td: 'px-2 py-1.5 align-top', base: 'border-separate border-spacing-0' }"
    />
  </div>
</template>
