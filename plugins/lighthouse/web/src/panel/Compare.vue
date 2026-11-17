<script setup lang="ts">
// Two runs of one breakpoint side by side (RB-091-02): the scores and Lighthouse's metrics of
// the older and the newer run, each with its change - the tool `reports` with `compare`. The
// breakpoint and the two runs are picked from the history; the two newest runs of the
// breakpoint are the default. The reports themselves open in the report window.
import { computed, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { breakpointsOf, defaultPair, formatDelta, formatMetric, runLabel, runsWith, type Comparison, type HistoryRun } from './history';
import { signed } from './model';

const props = defineProps<{ runs: HistoryRun[] }>();
const panel = usePanel();
const t = panel.t;

const breakpoint = ref<string | undefined>(undefined);
const before = ref<string | undefined>(undefined);
const after = ref<string | undefined>(undefined);
const result = ref<Comparison | null>(null);
const failure = ref<string | null>(null);
const busy = ref(false);

const breakpointItems = computed(() => breakpointsOf(props.runs));
const runItems = computed(() =>
  breakpoint.value ? runsWith(props.runs, breakpoint.value).map((run) => ({ label: runLabel(run.runId), value: run.runId })) : [],
);
const ready = computed(() => Boolean(breakpoint.value && before.value && after.value && before.value !== after.value));

// A new history keeps the choice while it still holds, else takes the defaults.
watch(
  breakpointItems,
  (items) => {
    if (!breakpoint.value || !items.includes(breakpoint.value)) breakpoint.value = items[0];
  },
  { immediate: true },
);
watch(
  [breakpoint, () => props.runs],
  () => {
    const offered = runItems.value.map((item) => item.value);
    if (before.value && after.value && offered.includes(before.value) && offered.includes(after.value)) return;
    const pair = breakpoint.value ? defaultPair(props.runs, breakpoint.value) : null;
    [before.value, after.value] = pair ?? [undefined, undefined];
    result.value = null;
  },
  { immediate: true },
);

async function compare(): Promise<void> {
  if (!ready.value) return;
  busy.value = true;
  failure.value = null;
  try {
    result.value = (await panel.call({ tool: 'reports' }, { action: 'compare', breakpoint: breakpoint.value, runs: [before.value, after.value] })) as Comparison;
  } catch (error) {
    result.value = null;
    failure.value = panel.error(error);
  } finally {
    busy.value = false;
  }
}

/** The metrics either run measured; one neither did is left out. */
const metricRows = computed(() => (result.value?.metrics ?? []).filter((row) => row.before !== null || row.after !== null));

const tone = (delta: number | null, higherIsBetter: boolean): string => {
  if (typeof delta !== 'number' || delta === 0) return 'text-muted';
  return delta > 0 === higherIsBetter ? 'text-success' : 'text-error';
};
</script>

<template>
  <section class="flex flex-col gap-2" aria-labelledby="compare-title">
    <h2 id="compare-title" class="flex items-center gap-1.5 text-xs font-semibold uppercase text-toned">
      <UIcon name="i-lucide-git-compare" class="size-4" />
      <span>{{ t('panel.compare.title') }}</span>
    </h2>
    <p v-if="breakpointItems.length === 0 || runItems.length < 2" id="compare-empty" class="text-sm text-muted">{{ t('panel.compare.empty') }}</p>
    <div class="grid grid-cols-1 gap-2 sm:grid-cols-3">
      <UFormField :label="t('panel.breakpoint')">
        <USelect id="compare-breakpoint" v-model="breakpoint" :items="breakpointItems" size="sm" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.compare.before')">
        <USelect id="compare-before" v-model="before" :items="runItems" size="sm" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.compare.after')">
        <USelect id="compare-after" v-model="after" :items="runItems" size="sm" class="w-full" />
      </UFormField>
    </div>
    <div>
      <UButton id="compare" size="sm" icon="i-lucide-git-compare" :label="t('panel.compare.run')" :disabled="!ready" :loading="busy" @click="compare" />
    </div>
    <UAlert v-if="failure" id="compare-failure" color="error" variant="subtle" icon="i-lucide-circle-x" :title="failure" :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }" />
    <!-- A native table: four columns of numbers with a row header each, no sorting, paging or
         selection - what UTable adds is not needed here (docs/design.md 5.1). -->
    <div v-if="result" class="overflow-x-auto">
      <table id="comparison" class="w-full text-sm tabular-nums">
        <caption class="sr-only">{{ t('panel.compare.caption', { breakpoint: result.breakpoint, before: runLabel(result.before), after: runLabel(result.after) }) }}</caption>
        <thead class="text-xs text-muted">
          <tr>
            <th scope="col" class="py-1 pe-2 text-start font-medium">{{ t('panel.compare.measure') }}</th>
            <th scope="col" class="py-1 pe-2 text-end font-medium">{{ t('panel.compare.before') }}</th>
            <th scope="col" class="py-1 pe-2 text-end font-medium">{{ t('panel.compare.after') }}</th>
            <th scope="col" class="py-1 text-end font-medium">{{ t('panel.compare.change') }}</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-default">
          <tr v-for="row in result.scores" :key="'score-' + row.category" data-score-row :data-category="row.category">
            <th scope="row" class="py-1 pe-2 text-start font-normal">{{ t('panel.categories.' + row.category) }}</th>
            <td class="py-1 pe-2 text-end">{{ row.before ?? '-' }}</td>
            <td class="py-1 pe-2 text-end">{{ row.after ?? '-' }}</td>
            <td data-change class="py-1 text-end" :class="tone(row.delta, true)">{{ typeof row.delta === 'number' ? signed(row.delta) : '' }}</td>
          </tr>
          <tr v-for="row in metricRows" :key="'metric-' + row.metric" data-metric-row :data-metric="row.metric">
            <th scope="row" class="py-1 pe-2 text-start font-normal">{{ t('panel.metrics.' + row.metric) }}</th>
            <td class="py-1 pe-2 text-end">{{ formatMetric(row.before, row.unit) }}</td>
            <td class="py-1 pe-2 text-end">{{ formatMetric(row.after, row.unit) }}</td>
            <td data-change class="py-1 text-end" :class="tone(row.delta, false)">{{ formatDelta(row.delta, row.unit) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
