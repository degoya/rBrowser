<script setup lang="ts">
// The wizard's picture of the grid (RB-090-43): the chosen views where the grid that is written
// puts them, at one common scale (`preview.ts`), each box centred over its label and size and
// standing on the row's baseline, the whole as wide as the figure. The figure is measured, so a
// narrower window gives a smaller picture of the same grid.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { PREVIEW, previewGeometry } from './preview';
import type { GridPreview } from './wizard';

const props = defineProps<{ grid: GridPreview; appended: boolean }>();
const { t } = useI18n();

const figure = ref<HTMLElement | null>(null);
const available = ref<number>(PREVIEW.fallbackWidth);
let observer: ResizeObserver | null = null;

onMounted(() => {
  const element = figure.value;
  if (element === null) {
    return;
  }
  if (element.clientWidth > 0) {
    available.value = element.clientWidth;
  }
  if (typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(([entry]) => {
      if (entry !== undefined && entry.contentRect.width > 0) {
        available.value = Math.floor(entry.contentRect.width);
      }
    });
    observer.observe(element);
  }
});
onBeforeUnmount(() => observer?.disconnect());

const geometry = computed(() => previewGeometry(props.grid, available.value));
</script>

<template>
  <figure ref="figure" class="flex min-w-0 flex-col gap-2 text-sm">
    <figcaption class="font-medium">
      {{ t(appended ? 'analyzer.wizard.preview_appended' : 'analyzer.wizard.preview') }}
    </figcaption>
    <div class="overflow-x-auto">
      <div
        class="grid items-end"
        :style="{
          gridTemplateColumns: geometry.trackWidths.map((track) => `${track}px`).join(' '),
          gap: `${PREVIEW.gap}px`,
          width: `${geometry.width}px`,
        }"
        data-testid="preview"
      >
        <div
          v-for="cell in geometry.cells"
          :key="cell.id"
          class="flex min-w-0 flex-col items-center gap-1"
          :style="{ gridColumn: String(cell.col), gridRow: String(cell.row) }"
          :title="t('analyzer.wizard.view', { label: cell.label, width: String(cell.width), height: String(cell.height) })"
          data-testid="preview-cell"
        >
          <div
            class="rounded-sm bg-primary/15 ring ring-primary/40"
            :style="{ width: `${cell.boxWidth}px`, height: `${cell.boxHeight}px` }"
            aria-hidden="true"
            data-testid="preview-box"
          />
          <span class="max-w-full truncate text-center text-xs font-medium text-highlighted" data-testid="preview-label">
            {{ cell.label }}
          </span>
          <span class="text-center text-xs text-muted tabular-nums">
            {{ t('analyzer.wizard.size', { width: String(cell.width), height: String(cell.height) }) }}
          </span>
        </div>
      </div>
    </div>
  </figure>
</template>
