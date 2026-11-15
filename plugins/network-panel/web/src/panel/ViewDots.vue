<script setup lang="ts">
// A dot per view of a request (RB-070-02, on Nuxt UI since RB-080-84), in the answer's view
// order: filled where the view loaded it, a ring where not; its shape tells the engine (round
// Chromium, square Firefox, diamond WebKit), its label the view, its status and size - the
// colour is never the only sign. Our own element: Nuxt UI has no per-view marker; a badge per
// view would not fit a row.
import { size, viewColor } from './model';
import type { ListRow, ListView } from './types';
import { useNetwork } from './store';

defineProps<{ row: ListRow; views: ListView[] }>();
const { panel } = useNetwork();
const t = panel.t;

function label(row: ListRow, view: ListView): string {
  const seen = row.views[String(view.id)];
  return seen
    ? t('list.viewSeen', { label: view.label, status: seen.status ?? seen.failed ?? '-', size: size(seen.size) })
    : t('list.viewMissing', { label: view.label });
}

const SHAPE: Record<string, string> = { firefox: 'rounded-[2px]', webkit: 'rotate-45 rounded-[1px]' };
</script>

<template>
  <span class="inline-flex items-center gap-1" data-dots>
    <span
      v-for="(view, index) in views"
      :key="view.id"
      role="img"
      data-dot
      :data-engine="view.engine ?? 'chromium'"
      :data-missing="row.views[String(view.id)] ? undefined : ''"
      :aria-label="label(row, view)"
      :title="label(row, view)"
      class="inline-block size-2.5 border-2"
      :class="SHAPE[view.engine ?? 'chromium'] ?? 'rounded-full'"
      :style="{ borderColor: viewColor(index), background: row.views[String(view.id)] ? viewColor(index) : 'transparent' }"
    />
  </span>
</template>
