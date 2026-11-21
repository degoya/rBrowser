<script setup lang="ts">
// A run live and its end (RB-096-07): the clock against the planned length, users running and
// wanted, requests per second, p50/p95/p99, failures with their share and by kind, and the time
// chart. While it runs, Stop - the same as the activity bar's - ends every connection at once.
// The figures are a grid of text, not a table: a handful of fixed values (docs/design.md 5.2).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import LoadChart from './LoadChart.vue';
import { type Snapshot, clock, lastRate, millis, percent } from './model';

const props = defineProps<{ snapshot: Snapshot; stopping: boolean }>();
const emit = defineEmits<{ stop: [] }>();
const t = usePanel().t;

const latency = computed(() => props.snapshot.totals.latency);
const kinds = computed(() => Object.entries(props.snapshot.errors).sort((a, b) => b[1] - a[1]));
const ended = computed(() => {
  const end = props.snapshot.end;
  if (!end) return null;
  const abort = props.snapshot.abort;
  if (end === 'aborted' && abort) {
    const value = abort.rule === 'errorRate' ? percent(abort.value) : millis(abort.value);
    const threshold = abort.rule === 'errorRate' ? percent(abort.threshold) : millis(abort.threshold);
    return t(`panel.live.abort.${abort.rule}`, { value, threshold });
  }
  return t(`panel.live.end.${end}`);
});
</script>

<template>
  <section id="live" class="flex flex-col gap-2" :aria-label="t('panel.live.title')" :data-running="snapshot.running ? 'true' : 'false'">
    <div class="flex flex-wrap items-center gap-2">
      <p class="text-sm font-medium">{{ snapshot.running ? t('panel.live.running', { name: snapshot.name || snapshot.scenario }) : t('panel.live.ended', { name: snapshot.name || snapshot.scenario }) }}</p>
      <span id="live-clock" class="font-mono text-sm text-muted">{{ clock(snapshot.elapsedS) }} / {{ clock(snapshot.durationS) }}</span>
      <UButton v-if="snapshot.running" id="stop" size="sm" color="error" icon="i-lucide-square" :loading="stopping" :label="t('panel.live.stop')" class="ms-auto" @click="emit('stop')" />
    </div>
    <p v-if="ended" id="live-end" role="status" class="text-sm">{{ ended }}</p>
    <p v-if="snapshot.reportId" id="live-report" class="text-xs text-muted">{{ t('panel.live.report', { id: snapshot.reportId }) }}</p>
    <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-sm sm:grid-cols-4">
      <div><dt class="text-xs text-muted">{{ t('panel.live.users') }}</dt><dd id="live-users">{{ t('panel.live.usersValue', { users: snapshot.users, target: snapshot.targetUsers }) }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.rate') }}</dt><dd id="live-rate">{{ t('panel.live.rateValue', { now: lastRate(snapshot), mean: Math.round(snapshot.totals.rps * 10) / 10 }) }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.requests') }}</dt><dd id="live-requests">{{ snapshot.totals.requests }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.failures') }}</dt><dd id="live-failures">{{ t('panel.live.failuresValue', { errors: snapshot.totals.errors, share: percent(snapshot.totals.errorRate) }) }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.p50') }}</dt><dd id="live-p50">{{ millis(latency?.p50) }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.p95') }}</dt><dd id="live-p95">{{ millis(latency?.p95) }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.p99') }}</dt><dd id="live-p99">{{ millis(latency?.p99) }}</dd></div>
      <div><dt class="text-xs text-muted">{{ t('panel.live.max') }}</dt><dd>{{ millis(latency?.max) }}</dd></div>
    </dl>
    <ul v-if="kinds.length > 0" id="live-kinds" class="flex flex-wrap gap-1" :aria-label="t('panel.live.kinds')">
      <li v-for="[kind, count] in kinds" :key="kind" :data-kind="kind">
        <UBadge size="sm" variant="subtle" color="error">{{ t(`panel.kind.${kind}`, { count }) }}</UBadge>
      </li>
    </ul>
    <LoadChart v-if="snapshot.series.length > 0" :series="snapshot.series" />
  </section>
</template>
