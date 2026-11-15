<script setup lang="ts">
// The split's divider in the toolbar strip (RB-080-04): the views' own gap cannot take the mouse
// (CEF Views panels get no mouse events, and the shell never covers a view), so the divider is
// moved here - a slider for the primary side's share, by mouse or the arrow keys, sent when it is
// let go. `Leader L Left` / `L Right` move it from anywhere.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import type { SplitState } from '../ipc/layout';

const props = defineProps<{ split: SplitState }>();
const emit = defineEmits<{ change: [ratio: number] }>();
const { t } = useI18n();

const percent = computed(() => Math.round(props.split.ratioPermille / 10));
const step = computed(() => Math.max(1, Math.round(props.split.stepPermille / 10)));
const label = computed(() =>
  t(props.split.vertical ? 'shell.toolbar.split_divider_vertical' : 'shell.toolbar.split_divider_horizontal'),
);

function commit(event: Event): void {
  const value = Number((event.target as HTMLInputElement).value);
  if (Number.isFinite(value)) emit('change', Math.min(90, Math.max(10, value)) / 100);
}
</script>

<template>
  <input
    type="range"
    min="10"
    max="90"
    :step="step"
    :value="percent"
    :aria-label="label"
    :aria-valuetext="t('shell.toolbar.split_share', { percent })"
    :title="label"
    class="h-4 w-20 shrink-0 cursor-ew-resize accent-(--ui-primary) focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-(--ui-primary)"
    data-testid="split-divider"
    @change="commit"
  />
</template>
