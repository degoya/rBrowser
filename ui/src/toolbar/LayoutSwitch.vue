<script setup lang="ts">
// The layout switcher (RB-010-12): the preset pulldown (the core's native menu, RB-090-45; only when the project has presets; the
// layout modes keep their fields in `layout.modes`, RB-080-04), focus mode on/off, in focus mode
// the quick switcher over the breakpoints (buttons and the left/right arrow keys while the
// toolbar has the focus), the layout modes (`ModeSwitch`, RB-080-04), "Save layout as preset"
// (its bar is `layout/SavePresetBar.vue`), the "+" that adds a hidden view (`AddViewMenu.vue`,
// RB-080-41) and zoom to fit. Every control runs a command of the registry.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { PRESET_ICONS } from '../presets/icons';
import { useFocusArrowKeys } from '../presets/focusKeys';
import { useShell } from '../state/shell';
import PulldownButton from './PulldownButton.vue';
import { choiceRows, shownChoice, type Choice } from './pulldown';
import ModeSwitch from '../layout/ModeSwitch.vue';
import AddViewMenu from './AddViewMenu.vue';

const shell = useShell();
const { t } = useI18n();

const layout = computed(() => shell.state.value?.layout);
const mode = computed(() => layout.value?.mode);
const focusMode = computed(() => mode.value === 'focus');
const choices = computed<Choice[]>(() => (layout.value?.presets ?? []).map((id) => ({ value: id, label: id })));
const preset = computed(() => layout.value?.preset ?? null);
const presetValue = computed(() =>
  shownChoice(choices.value, preset.value, t('shell.toolbar.layout_unset'), t('shell.toolbar.layout_none')),
);
const presetRows = () => choiceRows(choices.value, preset.value);
const focusLabel = computed(() => {
  const id = layout.value?.focus;
  const breakpoint = shell.state.value?.breakpoints.find((b) => b.id === id);
  return breakpoint?.label ?? id ?? '';
});

useFocusArrowKeys(mode, shell.run);
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5">
    <PulldownButton
      v-if="choices.length > 0"
      kind="layout"
      :label="t('shell.toolbar.layout')"
      :value="presetValue"
      :rows="presetRows"
      data-testid="layout"
      @choose="(id) => shell.run({ command: 'layout.select_preset', id })"
    />
    <ModeSwitch />
    <AddViewMenu />
    <UButton
      :icon="PRESET_ICONS.savePreset"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :aria-label="t('shell.toolbar.save_preset')"
      :title="t('shell.toolbar.save_preset')"
      data-testid="save-preset-open"
      @click="shell.run({ command: 'layout.save_preset' })"
    />
    <UButton
      :icon="PRESET_ICONS.focusMode"
      color="neutral"
      :variant="focusMode ? 'soft' : 'ghost'"
      size="sm"
      square
      :aria-pressed="focusMode"
      :aria-label="t('shell.toolbar.focus_mode')"
      :title="t('shell.toolbar.focus_mode')"
      data-testid="focus-mode"
      @click="shell.run({ command: 'layout.focus_mode' })"
    />
    <template v-if="focusMode">
      <UButton
        :icon="PRESET_ICONS.focusPrev"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :aria-label="t('shell.toolbar.focus_prev')"
        :title="t('shell.toolbar.focus_prev')"
        data-testid="focus-prev"
        @click="shell.run({ command: 'layout.focus_prev' })"
      />
      <span class="max-w-28 truncate text-xs text-muted" data-testid="focus-current">
        {{ t('shell.toolbar.focus_current', { label: focusLabel }) }}
      </span>
      <UButton
        :icon="PRESET_ICONS.focusNext"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        :aria-label="t('shell.toolbar.focus_next')"
        :title="t('shell.toolbar.focus_next')"
        data-testid="focus-next"
        @click="shell.run({ command: 'layout.focus_next' })"
      />
    </template>
    <UButton
      :icon="PRESET_ICONS.zoomToFit"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :aria-label="t('shell.toolbar.zoom_to_fit')"
      :title="t('shell.toolbar.zoom_to_fit')"
      data-testid="zoom-to-fit"
      @click="shell.run({ command: 'layout.zoom_to_fit' })"
    />
  </div>
</template>
