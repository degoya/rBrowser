<script setup lang="ts">
// A dot per view (RB-060-02, on Nuxt UI since RB-080-84), coloured by the view's place in the
// answer's view list: its shape tells the engine (round Chromium, square Firefox, diamond
// WebKit), its label the view, breakpoint, engine and how often it reported - the colour is never
// the only sign. Our own element: Nuxt UI has no per-view marker; a badge per view would not fit
// a row.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type ViewEntry, viewColor } from './model';

const props = defineProps<{ ids: number[]; views: ViewEntry[]; perView?: Record<string, number> }>();
const { t } = usePanel();

const SHAPE: Record<string, string> = { firefox: 'rounded-[2px]', webkit: 'rotate-45 rounded-[1px]' };

const dots = computed(() =>
  props.ids.map((id) => {
    const index = props.views.findIndex((view) => view.id === id);
    const view = props.views[index];
    const engine = view?.engine ?? 'unknown';
    return {
      id,
      engine,
      color: viewColor(index < 0 ? id : index),
      label: t('panel.viewDot', { label: view?.label ?? String(id), breakpoint: view?.breakpoint ?? '', engine: view?.engine ?? '', count: props.perView?.[String(id)] ?? 1 }),
    };
  }),
);
</script>

<template>
  <span class="inline-flex shrink-0 items-center gap-1" data-dots>
    <span
      v-for="dot in dots"
      :key="dot.id"
      role="img"
      data-dot
      :data-engine="dot.engine"
      :aria-label="dot.label"
      :title="dot.label"
      class="inline-block size-2.5"
      :class="SHAPE[dot.engine] ?? 'rounded-full'"
      :style="{ background: dot.color }"
    />
  </span>
</template>
