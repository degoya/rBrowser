<script setup lang="ts">
// The benchmark of the open saved request (RB-096-04): sent `api-client.benchRepeats` times in
// the chosen environment and, when one is picked, in a second one to compare - staging against
// production -, at most `api-client.benchConcurrency` at once, only to hosts the project allows.
// The header starts it (`RequestHeader.vue`, RB-096-11); here, below the tabs, each phase as
// min, median, p95 and max. The figures are a grid of text with table roles, not a UTable: four
// fixed columns, nothing to sort, select or page (docs/design.md 5.2).
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import SettingsText from './SettingsText.vue';
import { PHASES, counted, delta } from './checks';
import { millis } from './model';
import type { BenchRun } from './runs';

defineProps<{ run: BenchRun }>();
const t = usePanel().t;

const STATS = ['min', 'median', 'p95', 'max'] as const;
</script>

<template>
  <section id="bench" class="flex flex-col gap-2" :aria-label="t('panel.bench.title')">
    <p class="text-xs text-muted">{{ t('panel.bench.title') }}</p>
    <p id="bench-hint" class="text-xs text-muted"><SettingsText text="panel.bench.hint" setting-key="api-client.benchRepeats" /></p>
    <p v-if="run.running" id="bench-running" class="text-xs text-muted" role="status">{{ t('panel.bench.running') }}</p>
    <p v-else-if="run.problem" id="bench-problem" class="text-xs text-error" role="alert">{{ run.problem }}</p>
    <template v-if="run.result && !run.running">
      <div v-for="(series, index) in run.result.series" :key="index" data-series :data-environment="series.environment ?? ''" class="flex flex-col gap-1">
        <div class="flex flex-wrap items-center gap-1.5 text-xs">
          <UBadge v-if="series.environment" color="neutral" variant="outline" size="sm" :label="series.environment" />
          <span>{{ t('panel.bench.sent', { count: series.sent, concurrency: run.result.concurrency }) }}</span>
          <span v-if="Object.keys(series.statuses).length > 0" class="font-mono text-muted">{{ counted(series.statuses) }}</span>
          <span v-if="Object.keys(series.errors).length > 0" class="text-error">{{ t('panel.bench.errors', { count: Object.values(series.errors).reduce((a, b) => a + b, 0) }) }}</span>
        </div>
        <div role="table" :aria-label="t('panel.bench.table')" class="grid grid-cols-[minmax(7rem,auto)_repeat(4,minmax(4rem,1fr))] gap-x-3 gap-y-0.5 text-xs tabular-nums">
          <div role="row" class="contents">
            <span role="columnheader" class="text-muted">{{ t('panel.bench.phase') }}</span>
            <span v-for="stat in STATS" :key="stat" role="columnheader" class="text-end text-muted">{{ t(`panel.bench.stats.${stat}`) }}</span>
          </div>
          <template v-for="phase in PHASES" :key="phase">
            <div v-if="series.phases[phase]" role="row" class="contents" :data-phase="phase">
              <span role="rowheader">{{ t(`panel.response.phases.${phase}`) }}</span>
              <span v-for="stat in STATS" :key="stat" role="cell" class="text-end">{{ millis(series.phases[phase]?.[stat]) }}</span>
            </div>
          </template>
        </div>
      </div>
      <p v-if="run.result.comparison" id="bench-comparison" class="text-sm">
        {{ t('panel.bench.comparison', { median: delta(run.result.comparison.medianDelta), p95: delta(run.result.comparison.p95Delta), first: run.result.series[0]?.environment ?? '', second: run.result.series[1]?.environment ?? '' }) }}
      </p>
    </template>
  </section>
</template>
