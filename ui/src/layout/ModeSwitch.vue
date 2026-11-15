<script setup lang="ts">
// The layout modes in the toolbar (RB-080-04): grid, horizontal, split and canvas as one group of
// toggle buttons, then the mode's own controls - the split's divider and swap, the strip's
// scroll. Every control runs a command of the registry; the canvas's minimap is
// `CanvasMinimap.vue`, placed by the toolbar at the start or end. The strip's Previous and Next
// disable themselves at either end; the focus then goes to the other one (RB-090-23).
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { PRESET_ICONS } from '../presets/icons';
import { useShell } from '../state/shell';
import type { LayoutModeCommand } from '../toolbar/actions';
import type { LayoutMode } from '../ipc/protocol';
import SplitDivider from './SplitDivider.vue';

const shell = useShell();
const { t } = useI18n();

const MODES: { mode: LayoutMode; command: LayoutModeCommand; icon: string; label: string }[] = [
  { mode: 'grid', command: 'layout.mode.grid', icon: PRESET_ICONS.modeGrid, label: 'shell.toolbar.mode_grid' },
  {
    mode: 'horizontal',
    command: 'layout.mode.horizontal',
    icon: PRESET_ICONS.modeHorizontal,
    label: 'shell.toolbar.mode_horizontal',
  },
  { mode: 'split', command: 'layout.mode.split', icon: PRESET_ICONS.modeSplit, label: 'shell.toolbar.mode_split' },
  { mode: 'canvas', command: 'layout.mode.canvas', icon: PRESET_ICONS.modeCanvas, label: 'shell.toolbar.mode_canvas' },
];

const layout = computed(() => shell.state.value?.layout);
const mode = computed(() => layout.value?.mode ?? 'grid');
const split = computed(() => layout.value?.modes?.split);
const strip = computed(() => layout.value?.modes?.strip);
const scrolls = computed(() => (strip.value?.lastFirst ?? 0) > 0);

const prevButton = ref<{ $el: HTMLElement } | null>(null);
const nextButton = ref<{ $el: HTMLElement } | null>(null);

// Before the strip moves, note which of the two has the focus; after it, if that one disabled
// itself, the other takes the focus.
watch(
  () => [strip.value?.first, strip.value?.lastFirst],
  async () => {
    const active = document.activeElement;
    const pair = [prevButton, nextButton];
    const from = pair.find((button) => button.value !== null && button.value.$el === active);
    const to = pair.find((button) => button !== from);
    if (from === undefined || to === undefined) {
      return;
    }
    await nextTick();
    if (from.value?.$el.matches(':disabled') === true) {
      to.value?.$el.focus();
    }
  },
  { flush: 'pre' },
);
</script>

<template>
  <div
    role="group"
    :aria-label="t('shell.toolbar.layout_modes')"
    class="flex shrink-0 items-center gap-0.5"
    data-testid="layout-modes"
  >
    <UButton
      v-for="entry in MODES"
      :key="entry.mode"
      :icon="entry.icon"
      color="neutral"
      :variant="mode === entry.mode ? 'soft' : 'ghost'"
      size="sm"
      square
      :aria-pressed="mode === entry.mode"
      :aria-label="t(entry.label)"
      :title="t(entry.label)"
      :data-testid="`mode-${entry.mode}`"
      @click="shell.run({ command: entry.command })"
    />
    <template v-if="mode === 'split' && split">
      <SplitDivider
        :split="split"
        @change="(ratio) => shell.run({ command: 'layout.mode.split', args: { ratio } })"
      />
      <UButton
        :icon="PRESET_ICONS.splitSwap"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :aria-label="t('shell.toolbar.split_swap')"
        :title="t('shell.toolbar.split_swap')"
        data-testid="split-swap"
        @click="shell.run({ command: 'layout.split.swap' })"
      />
    </template>
    <template v-if="mode === 'horizontal' && strip && scrolls">
      <UButton
        ref="prevButton"
        :icon="PRESET_ICONS.focusPrev"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :disabled="strip.first === 0"
        :aria-label="t('shell.toolbar.strip_prev')"
        :title="t('shell.toolbar.strip_prev')"
        data-testid="strip-prev"
        @click="shell.run({ command: 'layout.focus_prev' })"
      />
      <span class="text-xs text-muted" data-testid="strip-position">
        {{ t('shell.toolbar.strip_position', { first: strip.first + 1, count: strip.count }) }}
      </span>
      <UButton
        ref="nextButton"
        :icon="PRESET_ICONS.focusNext"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :disabled="strip.first >= strip.lastFirst"
        :aria-label="t('shell.toolbar.strip_next')"
        :title="t('shell.toolbar.strip_next')"
        data-testid="strip-next"
        @click="shell.run({ command: 'layout.focus_next' })"
      />
    </template>
  </div>
</template>
