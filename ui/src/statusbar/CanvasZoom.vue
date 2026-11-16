<script setup lang="ts">
// The canvas zoom at the right of the status bar (RB-080-94, `canvasZoom.ts`): only while the
// canvas layout is shown and `ui.statusBar.canvasZoom` is on. Minus, a slider, plus, and the
// level as a button whose presets (Fit, 25 to 200 %) the core opens as a native menu above it
// (RB-090-18): the status bar is a CEF view exactly as high as its content, so its popup is
// native (`docs/design.md` section 2), and a `<select>` opened its list below the bar - off the
// screen with the bar at its bottom.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import USlider from '@nuxt/ui/components/Slider.vue';
import type { IpcClient } from '../ipc/client';
import { STATUS_ICONS } from './icons';
import { MAX_PERCENT, MIN_PERCENT, STEP_PERCENT, clampPercent, createCanvasZoom } from './canvasZoom';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const zoom = createCanvasZoom(props.client);

/** The slider's value: the core's level, or where the slider is held while it is dragged. */
const slider = ref(100);
watch(
  () => zoom.percent.value,
  (percent) => {
    if (percent !== null) slider.value = clampPercent(percent);
  },
  { immediate: true },
);

const level = computed(() => t('status.canvas_zoom.level', { percent: zoom.percent.value ?? 0 }));
const presetsLabel = computed(() => t('status.canvas_zoom.presets', { percent: zoom.percent.value ?? 0 }));

function onSlide(value: number | undefined): void {
  if (typeof value === 'number') slider.value = value;
}

/** Sends the level where the slider was let go. Reka commits a key step (`change`) before the
 *  new value reaches the model, and Nuxt UI's `change` carries no value: the level is read a
 *  tick later, or an arrow key sent the one before its step (RB-090-45). */
async function commit(): Promise<void> {
  await nextTick();
  zoom.setPercent(slider.value);
}

/** The keys that opened the select's list (Alt or not): they open the presets' menu. */
const OPEN_KEYS = new Set(['ArrowDown', 'ArrowUp', 'F4']);

function openPresets(event: Event): void {
  const button = event.currentTarget;
  if (button instanceof HTMLElement) zoom.openPresets(button.getBoundingClientRect());
}

function onLevelKey(event: KeyboardEvent): void {
  if (!OPEN_KEYS.has(event.key)) return;
  event.preventDefault();
  openPresets(event);
}

onMounted(() => void zoom.start());
onBeforeUnmount(() => zoom.stop());
</script>

<template>
  <div
    v-if="zoom.visible.value"
    role="group"
    :aria-label="t('status.canvas_zoom.label')"
    class="flex shrink-0 items-center gap-1"
    data-testid="canvas-zoom"
  >
    <UButton
      :icon="STATUS_ICONS.canvasZoomOut"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      :aria-label="t('status.canvas_zoom.zoom_out')"
      :title="t('status.canvas_zoom.zoom_out')"
      data-testid="canvas-zoom-out"
      @click="zoom.step(false)"
    />
    <USlider
      :model-value="slider"
      :min="MIN_PERCENT"
      :max="MAX_PERCENT"
      :step="STEP_PERCENT"
      size="xs"
      class="w-24"
      :aria-label="t('status.canvas_zoom.slider')"
      data-testid="canvas-zoom-slider"
      @update:model-value="onSlide"
      @change="commit"
    />
    <UButton
      :icon="STATUS_ICONS.canvasZoomIn"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      :aria-label="t('status.canvas_zoom.zoom_in')"
      :title="t('status.canvas_zoom.zoom_in')"
      data-testid="canvas-zoom-in"
      @click="zoom.step(true)"
    />
    <UButton
      :label="level"
      :trailing-icon="STATUS_ICONS.canvasZoomPresets"
      color="neutral"
      variant="outline"
      size="xs"
      class="tabular-nums"
      aria-haspopup="menu"
      :aria-label="presetsLabel"
      :title="presetsLabel"
      data-testid="canvas-zoom-level"
      @click="openPresets"
      @keydown="onLevelKey"
    />
  </div>
</template>
