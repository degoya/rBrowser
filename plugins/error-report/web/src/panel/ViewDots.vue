<script setup lang="ts">
// A dot per view that reported an entry (RB-080-09, on Nuxt UI since RB-080-84): its colour is
// the view's, its shape the engine's (round Chromium, square Firefox, diamond WebKit), its label
// names the view, breakpoint, engine and count - the colour is never the only sign. Our own
// element: Nuxt UI has no per-view marker, and a badge per view would not fit a row.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type ReportView, viewColor } from './model';

const props = defineProps<{ ids: number[]; perView: Record<string, number>; views: ReportView[] }>();
const { t } = usePanel();

const SHAPE: Record<string, string> = { firefox: 'rounded-[1px]', webkit: 'rotate-45 scale-85 rounded-[1px]' };

const dots = computed(() =>
  props.ids.map((id) => {
    const index = props.views.findIndex((view) => view.id === id);
    const view = props.views[index];
    const engine = view?.engine ?? 'unknown';
    return {
      id,
      engine,
      color: viewColor(index >= 0 ? index : id),
      label: t('panel.viewDot', { label: view?.label ?? String(id), breakpoint: view?.breakpoint ?? '', engine: view?.engine ?? '', count: props.perView[String(id)] ?? 1 }),
    };
  }),
);
</script>

<template>
  <span class="inline-flex items-center gap-1" data-dots>
    <span
      v-for="dot in dots"
      :key="dot.id"
      role="img"
      data-dot
      :data-engine="dot.engine"
      :aria-label="dot.label"
      :title="dot.label"
      class="inline-block size-2.5 shrink-0"
      :class="SHAPE[dot.engine] ?? 'rounded-full'"
      :style="{ background: dot.color }"
    />
  </span>
</template>
