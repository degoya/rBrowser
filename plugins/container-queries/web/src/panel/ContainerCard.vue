<script setup lang="ts">
// One container of the analysis (RB-094-01): its selector with its names and type as badges, a
// table of its size per view - the next threshold below and above, the conditions that match
// there, a width to try (the keyboard's way to the overlay's drag handle) and the reset of a
// width rBrowser set - and the breakpoints that switch it.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Analysis, type Container, type ContainerView, conditionText, nextText, sizeText, switchRows } from './model';

const props = defineProps<{ container: Container; analysis: Analysis; busy: boolean }>();
const emit = defineEmits<{ reset: [view: number]; resize: [view: number, width: number] }>();
const { t } = usePanel();

const rows = computed(() => props.container.views);
const hasNext = computed(() => rows.value.some((view) => nextText(view.inline) || nextText(view.block)));
const columns = computed(() => [
  { id: 'view', header: t('panel.columns.view') },
  { id: 'size', header: t('panel.columns.size'), meta: { class: { th: 'text-right', td: 'text-right tabular-nums' } } },
  ...(hasNext.value ? [{ id: 'next', header: t('panel.columns.next'), meta: { class: { td: 'tabular-nums whitespace-pre' } } }] : []),
  { id: 'matching', header: t('panel.columns.matching') },
  { id: 'width', header: t('panel.columns.width'), meta: { class: { td: 'w-28' } } },
]);
const switches = computed(() => switchRows(props.container, props.analysis));
const matching = (view: ContainerView): string[] => view.matching.map((id) => conditionText(props.analysis.conditions[id]));
/** Tries the width typed into a row's field, on Enter. */
function apply(view: ContainerView, event: Event): void {
  const width = Number((event.target as HTMLInputElement).value);
  if (Number.isFinite(width) && width >= 0 && width <= 100_000) emit('resize', view.view, width);
}
const next = (view: ContainerView): string =>
  [nextText(view.inline), nextText(view.block) ? `${t('panel.block')} ${nextText(view.block)}` : ''].filter(Boolean).join('  ');
</script>

<template>
  <section class="flex flex-col gap-2 rounded-md border border-default p-2" data-container :aria-label="container.selector">
    <header class="flex flex-wrap items-center gap-1.5">
      <code class="text-sm font-semibold text-highlighted">{{ container.selector }}</code>
      <UBadge v-for="name in container.names" :key="name" data-name color="primary" variant="subtle" size="sm" :label="name" />
      <UBadge data-type color="neutral" variant="outline" size="sm" :label="container.type" />
    </header>
    <UTable :data="rows" :columns="columns" class="text-sm">
      <template #view-cell="{ row }">
        <span class="flex items-center gap-1.5">
          <span>{{ (row.original as ContainerView).label || `#${(row.original as ContainerView).view}` }}</span>
          <UBadge v-if="(row.original as ContainerView).resized" data-resized color="warning" variant="subtle" size="sm" icon="i-lucide-move-horizontal" :label="t('panel.resized')" />
        </span>
      </template>
      <template #size-cell="{ row }">
        {{ sizeText((row.original as ContainerView).inline.size) }} × {{ sizeText((row.original as ContainerView).block.size) }}
      </template>
      <template #next-cell="{ row }">{{ next(row.original as ContainerView) }}</template>
      <template #matching-cell="{ row }">
        <span class="flex flex-wrap items-center gap-1">
          <code v-for="text in matching(row.original as ContainerView)" :key="text" data-match class="text-xs">{{ text }}</code>
        </span>
      </template>
      <template #width-cell="{ row }">
        <span class="flex items-center gap-1">
          <UInput
            data-width
            type="number"
            size="xs"
            class="w-20"
            :min="0"
            :max="100000"
            :model-value="String(Math.round((row.original as ContainerView).inline.size))"
            :aria-label="t('panel.width', { view: (row.original as ContainerView).label || `#${(row.original as ContainerView).view}` })"
            :disabled="busy"
            @keydown.enter="apply(row.original as ContainerView, $event)"
          />
          <UTooltip v-if="(row.original as ContainerView).resized" :text="t('panel.reset')">
            <UButton
              data-reset
              icon="i-lucide-undo-2"
              color="neutral"
              variant="ghost"
              size="xs"
              :aria-label="t('panel.reset')"
              :disabled="busy"
              @click="emit('reset', (row.original as ContainerView).view)"
            />
          </UTooltip>
        </span>
      </template>
    </UTable>
    <ul v-if="switches.length > 0" class="flex flex-col gap-0.5 text-xs text-muted" data-switches>
      <li v-for="entry in switches" :key="`${entry.from}-${entry.to}`">
        {{ t('panel.switch', { from: entry.from, to: entry.to }) }}
        <span v-if="entry.gained.length > 0">{{ t('panel.gains', { conditions: entry.gained.join(', ') }) }}</span>
        <span v-if="entry.lost.length > 0">{{ t('panel.loses', { conditions: entry.lost.join(', ') }) }}</span>
      </li>
    </ul>
  </section>
</template>
