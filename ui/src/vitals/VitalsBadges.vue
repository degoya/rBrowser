<script setup lang="ts">
// The Web Vitals and resources of one view (RB-060-10), in its cell's label strip - the grid gap
// above the view, never over it. One badge per measured vital: shape, name and value
// ("● LCP 1.84 s"), the rating in words in its accessible name and tooltip, never colour alone;
// a click marks the element behind it in the view. Then the resources ("CPU 12 % · 48 MB",
// "high" and a triangle above a threshold) and the throttling. The label view is one line high,
// so the tooltip is the native `title`; the same text is each badge's accessible name, reached
// with Tab.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { ViewMetrics, VitalMetric, VitalReading } from '../ipc/protocol';
import { formatResource, formatVital, SHAPES, TONES } from './format';

const props = defineProps<{ metrics: ViewMetrics | null }>();
const emit = defineEmits<{ highlight: [view: number, metric: VitalMetric] }>();
const { t } = useI18n();

const measured = computed(() =>
  (props.metrics?.vitals ?? []).filter(
    (reading): reading is VitalReading & { value: number; rating: NonNullable<VitalReading['rating']> } =>
      reading.value !== undefined && reading.rating !== undefined,
  ),
);
const missing = computed(() =>
  (props.metrics?.vitals ?? [])
    .filter((reading) => reading.value === undefined)
    .map((reading) =>
      t('shell.vitals.missing', {
        metric: t(`shell.vitals.metric.${reading.metric}`),
        reason: t(`shell.vitals.unavailable.${reading.unavailable ?? 'pending'}`),
      }),
    )
    .join('\n'),
);

function name(reading: VitalReading & { value: number }): string {
  const rating = reading.rating ?? 'good';
  const text = t('shell.vitals.badge', {
    metric: t(`shell.vitals.metric.${reading.metric}`),
    value: formatVital(reading.metric, reading.value),
    rating: t(`shell.vitals.rating.${rating}`),
  });
  const lines = [text];
  if (props.metrics?.soft) lines.push(t('shell.vitals.soft'));
  if (reading.highlight) lines.push(t('shell.vitals.highlight'));
  if (missing.value !== '') lines.push(missing.value);
  return lines.join('\n');
}

function mark(reading: VitalReading): void {
  if (props.metrics !== null && reading.highlight) emit('highlight', props.metrics.view, reading.metric);
}

const resources = computed(() => props.metrics?.resources ?? null);
const warn = computed(() => resources.value?.cpuWarn === true || resources.value?.ramWarn === true);
const resourceText = computed(() => {
  const reading = resources.value;
  if (reading === null) return '';
  if (reading.unavailable !== undefined) {
    return t('shell.vitals.resources.na', { reason: t(`shell.vitals.unavailable.${reading.unavailable}`) });
  }
  const cpu = reading.cpuPercent === undefined ? '-' : formatResource(reading.cpuPercent);
  const memory = reading.memoryMb === undefined ? '-' : formatResource(reading.memoryMb);
  return t('shell.vitals.resources.reading', { cpu, memory });
});
const resourceHint = computed(() => {
  const reading = resources.value;
  if (reading === null) return '';
  const lines = [t('shell.vitals.resources.hint')];
  if (reading.cpuWarn) lines.push(t('shell.vitals.resources.cpuWarn'));
  if (reading.ramWarn) lines.push(t('shell.vitals.resources.ramWarn'));
  return lines.join('\n');
});
const throttle = computed(() => {
  const state = props.metrics?.throttle;
  if (state === undefined) return '';
  return state.state === 'hidden'
    ? t('shell.vitals.throttle.hidden')
    : t('shell.vitals.throttle.cpuLimited', { fps: state.fps });
});
</script>

<template>
  <span v-if="metrics !== null" class="flex shrink-0 items-center gap-1 tabular-nums" data-testid="vitals-badges">
    <button
      v-for="reading in measured"
      :key="reading.metric"
      type="button"
      class="flex items-center gap-0.5 rounded-sm px-0.5 focus-visible:outline-2 focus-visible:outline-primary"
      :class="reading.highlight ? 'cursor-pointer' : 'cursor-default'"
      :title="name(reading)"
      :aria-label="name(reading)"
      :data-testid="`vital-${reading.metric}`"
      :data-rating="reading.rating"
      @click="mark(reading)"
    >
      <span aria-hidden="true" :class="TONES[reading.rating]">{{ SHAPES[reading.rating] }}</span>
      <span aria-hidden="true">{{ t(`shell.vitals.metric.${reading.metric}`) }}</span>
      <span aria-hidden="true" class="text-default">{{ formatVital(reading.metric, reading.value) }}</span>
    </button>
    <span
      v-if="resources !== null"
      class="flex items-center gap-0.5 rounded-sm px-0.5"
      :class="warn ? 'bg-error/15 font-medium text-error' : ''"
      :title="resourceHint"
      :aria-label="warn ? `${resourceText}, ${t('shell.vitals.resources.high')}. ${resourceHint}` : `${resourceText}. ${resourceHint}`"
      role="status"
      tabindex="0"
      data-testid="vital-resources"
    >
      <span v-if="warn" aria-hidden="true">{{ SHAPES.poor }}</span>
      <span aria-hidden="true">{{ resourceText }}</span>
      <span v-if="warn" aria-hidden="true">{{ t('shell.vitals.resources.high') }}</span>
    </span>
    <span v-if="throttle !== ''" class="text-dimmed" data-testid="vital-throttle">{{ throttle }}</span>
  </span>
</template>
