<script setup lang="ts">
// The runs' reports (RB-096-07), newest first: when, how it ended, requests, rate, failures and
// p95. Two ticked are compared side by side, with what changed from the older to the newer.
// Each report is also a page of its own in the project, `.rbrowser/out/load-test/<id>.html`.
import { computed, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type Compared, type ReportRow, millis, percent } from './model';

const props = defineProps<{ reports: ReportRow[] }>();
const panel = usePanel();
const t = panel.t;

const picked = ref<string[]>([]);
const compared = ref<Compared | null>(null);
const problem = ref<string | null>(null);

const canCompare = computed(() => picked.value.length === 2);

function toggle(id: string, on: boolean | 'indeterminate'): void {
  const rest = picked.value.filter((known) => known !== id);
  picked.value = on === true ? [...rest, id].slice(-2) : rest;
}

async function compare(): Promise<void> {
  if (!canCompare.value) return;
  // The older one first: its id starts with the earlier second.
  const [a, b] = [...picked.value].sort();
  problem.value = null;
  try {
    compared.value = (await panel.call({ tool: 'panel' }, { op: { op: 'compare', a, b } })) as Compared;
  } catch (error) {
    compared.value = null;
    problem.value = panel.error(error);
  }
}

const ROWS = ['requests', 'rps', 'errorRate', 'p50', 'p95', 'p99'] as const;
function shown(key: (typeof ROWS)[number], value: number | null): string {
  if (value === null) return '-';
  if (key === 'errorRate') return percent(value);
  if (key === 'requests') return String(value);
  if (key === 'rps') return String(Math.round(value * 10) / 10);
  return millis(value);
}
</script>

<template>
  <section id="reports" class="flex flex-col gap-1" :aria-label="t('panel.reports.title')">
    <p class="text-xs text-muted">{{ t('panel.reports.title') }}</p>
    <p v-if="reports.length === 0" id="reports-empty" class="text-xs text-muted">{{ t('panel.reports.none') }}</p>
    <div v-for="row in props.reports.slice(0, 20)" :key="row.id" class="flex items-center gap-2 text-xs" :data-report="row.id">
      <UCheckbox :model-value="picked.includes(row.id)" :aria-label="t('panel.reports.pick', { id: row.id })" @update:model-value="(on) => toggle(row.id, on)" />
      <span class="min-w-0 flex-1 truncate">{{ row.name || row.scenario }} · {{ row.startedAt }}</span>
      <span class="text-muted">{{ t(`panel.live.end.${row.end}`) }}</span>
      <span class="font-mono">{{ t('panel.reports.row', { requests: row.requests, share: percent(row.errorRate), p95: millis(row.p95) }) }}</span>
    </div>
    <UButton v-if="reports.length > 1" id="compare" size="xs" color="neutral" variant="outline" icon="i-lucide-git-compare" class="self-start" :disabled="!canCompare" :label="t('panel.reports.compare')" @click="compare" />
    <p v-if="problem" role="alert" class="text-xs text-error">{{ problem }}</p>
    <div v-if="compared" id="compared" role="table" class="grid grid-cols-4 gap-x-3 text-xs" :aria-label="t('panel.reports.compared')">
      <div role="row" class="contents font-medium">
        <span role="columnheader" />
        <span role="columnheader" class="truncate">{{ compared.a.startedAt }}</span>
        <span role="columnheader" class="truncate">{{ compared.b.startedAt }}</span>
        <span role="columnheader">{{ t('panel.reports.change') }}</span>
      </div>
      <div v-for="key in ROWS" :key="key" role="row" class="contents" :data-compare="key">
        <span role="rowheader" class="text-muted">{{ t(`panel.reports.figure.${key}`) }}</span>
        <span role="cell">{{ shown(key, compared.a[key]) }}</span>
        <span role="cell">{{ shown(key, compared.b[key]) }}</span>
        <span role="cell">{{ compared.delta[key] === null ? '-' : `${(compared.delta[key] ?? 0) > 0 ? '+' : ''}${shown(key, compared.delta[key])}` }}</span>
      </div>
    </div>
  </section>
</template>
