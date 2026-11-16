<script setup lang="ts">
// The split's divider in the toolbar strip (RB-080-04): the views' own gap cannot take the mouse
// (CEF Views panels get no mouse events, and the shell never covers a view), so the divider is
// moved here - a slider for the primary side's share, by mouse or the arrow keys, sent when it is
// let go. `Leader L Left` / `L Right` move it from anywhere. A `USlider` since RB-080-88: it holds
// the share while it is dragged and sends it on `change`, which reka fires on release and on
// every key step - for a key step before the new value reaches the model, and Nuxt UI's `change`
// event carries no value, so the share is read a tick later.
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import USlider from '@nuxt/ui/components/Slider.vue';
import type { SplitState } from '../ipc/layout';

const props = defineProps<{ split: SplitState }>();
const emit = defineEmits<{ change: [ratio: number] }>();
const { t } = useI18n();

const percent = computed(() => Math.round(props.split.ratioPermille / 10));
const step = computed(() => Math.max(1, Math.round(props.split.stepPermille / 10)));
const label = computed(() =>
  t(props.split.vertical ? 'shell.toolbar.split_divider_vertical' : 'shell.toolbar.split_divider_horizontal'),
);

/** The share on the slider: the core's, or where the slider is held while it is dragged. */
const held = ref(percent.value);
watch(percent, (value) => (held.value = value));

function slide(value: number | number[] | undefined): void {
  const next = Array.isArray(value) ? value[0] : value;
  if (next !== undefined && Number.isFinite(next)) {
    held.value = next;
  }
}

async function commit(): Promise<void> {
  await nextTick();
  emit('change', Math.min(90, Math.max(10, held.value)) / 100);
}
</script>

<template>
  <USlider
    :model-value="held"
    :min="10"
    :max="90"
    :step="step"
    size="xs"
    class="w-20 shrink-0 cursor-ew-resize"
    :aria-label="label"
    :aria-valuetext="t('shell.toolbar.split_share', { percent: held })"
    :title="label"
    data-testid="split-divider"
    @update:model-value="slide"
    @change="commit"
  />
</template>
