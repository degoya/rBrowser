<script setup lang="ts">
// The run over time (RB-096-07): requests per second (solid) and p95 (dashed) per second. An own
// SVG, not a Nuxt UI component: Nuxt UI has no chart (docs/design.md 5.1); the colours are theme
// tokens and the legend names each line in words, never by colour alone.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { line } from './chart';
import { millis, type Point } from './model';

const props = defineProps<{ series: Point[] }>();
const t = usePanel().t;
const WIDTH = 600;
const HEIGHT = 120;

const rate = computed(() => line(props.series.map((point) => point.requests), WIDTH, HEIGHT));
const p95 = computed(() => line(props.series.map((point) => point.p95), WIDTH, HEIGHT));
</script>

<template>
  <figure id="load-chart" class="flex flex-col gap-1">
    <svg :viewBox="`0 0 ${WIDTH} ${HEIGHT}`" class="h-28 w-full rounded border border-default bg-elevated" preserveAspectRatio="none" role="img" :aria-label="t('panel.live.chart')">
      <polyline data-line="rate" fill="none" stroke="currentColor" class="text-primary" stroke-width="2" vector-effect="non-scaling-stroke" :points="rate.points" />
      <polyline data-line="p95" fill="none" stroke="currentColor" class="text-warning" stroke-width="2" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" :points="p95.points" />
    </svg>
    <figcaption class="flex flex-wrap gap-x-4 text-xs text-muted">
      <span>{{ t('panel.live.legendRate', { top: Math.round(rate.top) }) }}</span>
      <span>{{ t('panel.live.legendP95', { top: millis(p95.top) }) }}</span>
    </figcaption>
  </figure>
</template>
