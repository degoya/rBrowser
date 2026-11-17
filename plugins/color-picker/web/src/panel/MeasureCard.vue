<script setup lang="ts">
// Characters per line in every view (RB-070-10, on Nuxt UI since RB-080-86): the selector (from
// the setting until the person types), Measure, Choose an element and Clear marks, the comfortable
// range, and a row per view with its rating in words and colour. No form: the frame has no
// `allow-forms`, so Enter in the field measures (RB-080-66).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import { RATING_COLOR, type Measured } from './model';

const props = defineProps<{ measured: Measured | null; failure: (code: string) => string }>();
const selector = defineModel<string>('selector', { required: true });
const emit = defineEmits<{ run: []; choose: []; clear: []; typed: [] }>();
const { t } = usePanel();

const hint = computed(() => {
  const measured = props.measured;
  if (!measured) return '';
  return t('panel.measure_hint', { selector: measured.selector, min: measured.optimal?.min, max: measured.optimal?.max, warn: measured.warnAbove });
});

const NUMBER = { th: 'text-right', td: 'text-right tabular-nums' };
const columns = computed(() => [
  { accessorKey: 'label', header: t('panel.view') },
  { accessorKey: 'lines', header: t('panel.lines'), meta: { class: NUMBER } },
  { accessorKey: 'average', header: t('panel.average'), meta: { class: NUMBER } },
  { accessorKey: 'max', header: t('panel.max'), meta: { class: NUMBER } },
  { accessorKey: 'over', header: t('panel.over'), meta: { class: NUMBER } },
  { id: 'rating', header: t('panel.rating_head') },
]);
const rows = computed(() =>
  (props.measured?.views ?? []).map((view) => ({
    label: String(view.label ?? view.view),
    lines: view.error ? '' : String(view.lines ?? ''),
    average: view.error ? '' : String(view.average ?? ''),
    max: view.error ? '' : String(view.max ?? ''),
    over: view.error ? '' : String(view.over ?? ''),
    rating: String(view.rating ?? ''),
    error: view.error ? props.failure(String(view.error)) : '',
  })),
);

function onKey(event: KeyboardEvent): void {
  if (event.key !== 'Enter') return;
  event.preventDefault();
  emit('run');
}

const CARD_UI = { header: 'flex items-center gap-2 px-3 py-2 sm:px-3', body: 'flex flex-col gap-2 p-3 sm:p-3' };
</script>

<template>
  <UCard as="section" aria-labelledby="measure-title" :ui="CARD_UI">
    <template #header>
      <h2 id="measure-title" class="min-w-0 flex-1 truncate text-sm font-semibold text-highlighted">{{ t('panel.measure') }}</h2>
      <IconAction id="measure-clear" icon="i-lucide-eraser" :label="t('panel.marks_clear')" @click="emit('clear')" />
      <IconAction id="measure-choose" icon="i-lucide-mouse-pointer-click" :label="t('panel.measure_choose')" @click="emit('choose')" />
    </template>
    <div id="measure-form" class="flex flex-wrap items-end gap-2">
      <UFormField :label="t('panel.selector')" name="selector" size="sm" class="min-w-0 flex-[1_1_200px]">
        <UInput
          id="selector"
          v-model="selector"
          class="w-full"
          :ui="{ base: 'font-mono' }"
          spellcheck="false"
          autocomplete="off"
          @input="emit('typed')"
          @keydown="onKey"
        />
      </UFormField>
      <UButton id="measure-run" type="button" size="sm" variant="soft" icon="i-lucide-ruler" :label="t('panel.measure_run')" @click="emit('run')" />
    </div>
    <p v-if="hint" id="measure-hint" class="text-xs text-muted">{{ hint }}</p>
    <UTable v-if="rows.length > 0" id="measure-table" :data="rows" :columns="columns" :ui="{ th: 'px-2 py-1.5', td: 'px-2 py-1.5' }" class="text-sm">
      <template #label-cell="{ row }">
        <span class="font-medium">{{ row.original.label }}</span>
      </template>
      <template #rating-cell="{ row }">
        <span v-if="row.original.error" data-error class="text-error">{{ row.original.error }}</span>
        <UBadge
          v-else
          data-rating
          :data-tone="RATING_COLOR[row.original.rating] === 'error' ? 'error' : RATING_COLOR[row.original.rating] === 'success' ? 'ok' : undefined"
          :color="RATING_COLOR[row.original.rating] ?? 'neutral'"
          variant="subtle"
          size="sm"
          :label="t(`panel.rating.${row.original.rating}`)"
        />
      </template>
    </UTable>
  </UCard>
</template>
