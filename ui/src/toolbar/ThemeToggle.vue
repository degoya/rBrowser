<script setup lang="ts">
// The choice of rBrowser's own theme in the toolbar (RB-080-95, RB-090-10): System, Light, Dark
// for the shell, the panels, the settings and every window, the native parts and the title bar
// included - never the pages in the views, whose colour scheme is the emulation's sun and moon.
// One icon in every state, a sun and a moon in one, so it reads as "light or dark" at a glance;
// a small mark on it (a screen, a sun with rays, a moon with stars) says the theme in effect,
// and its name and tooltip say it in words - for System with what Windows uses now.
//
// A native select like the toolbar's other menus (`OverflowMenu.vue`, `docs/design.md` section
// 2): CEF opens its list as a native popup in front of the views, where a Nuxt UI menu would be
// cut off at the edge of the 44 px strip. A choice from the list runs `ui.theme.set`; a key
// that only browses the closed select opens the list or sets nothing (`menuKeys.ts`). The
// tooltip names the shortcut that steps through the three (`ui.theme.cycle`, `Leader V 0`), from
// the bindings the shell keeps current. `ui.toolbar.themeToggle` off: nothing is drawn. In a
// narrow window its group leaves the row and the overflow menu shows the three choices under its
// name.
import { computed, onMounted, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UIcon from '@nuxt/ui/components/Icon.vue';
import { booleanSetting } from '../settings/booleanSetting';
import { createMenuKeys } from './menuKeys';
import { useShell } from '../state/shell';
import { parseTheme } from '../theme/theme';
import {
  THEME_CHOICES,
  THEME_CYCLE_COMMAND,
  THEME_ICON,
  THEME_MARKS,
  THEME_TOGGLE_SETTING,
  themeChoiceKey,
  themeLabelKey,
} from './themeToggle';

const shell = useShell();
const { t } = useI18n();
const id = useId();

const shown = computed(() => booleanSetting(shell.settings.value[THEME_TOGGLE_SETTING], true));
const label = computed(() => t(themeLabelKey(shell.theme.value, shell.systemTheme.value)));
const title = computed(() => {
  const shortcut = shell.bindingOf(THEME_CYCLE_COMMAND);
  return shortcut === null
    ? t('shell.toolbar.theme.hint', { label: label.value })
    : t('shell.toolbar.theme.hint_shortcut', { label: label.value, shortcut });
});
const choices = computed(() =>
  THEME_CHOICES.map((theme) => {
    const text = t(themeChoiceKey(theme, shell.systemTheme.value));
    return {
      value: theme,
      label: theme === shell.theme.value ? t('shell.toolbar.theme.option.current', { label: text }) : text,
    };
  }),
);

/** An arrow or a typed letter on the closed select opens its list or sets nothing (`menuKeys.ts`). */
const keys = createMenuKeys();

function onChange(event: Event): void {
  const element = event.target as HTMLSelectElement;
  const chosen = element.value;
  // The select names the menu, not a choice: it goes back to its label, so the same choice can
  // be made again.
  element.value = '';
  if (chosen !== '' && !keys.browsing()) {
    void shell.run({ command: 'ui.theme.set', theme: parseTheme(chosen) });
  }
}

onMounted(() => shell.loadCommands());
</script>

<template>
  <div v-if="shown" class="relative flex shrink-0 items-center">
    <label :for="id" class="sr-only">{{ label }}</label>
    <select
      :id="id"
      :aria-label="label"
      :title="title"
      value=""
      class="h-8 w-8 cursor-pointer appearance-none rounded-md bg-transparent pr-0 pl-8 text-sm text-default hover:bg-elevated focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
      data-testid="theme-toggle"
      :data-theme-setting="shell.theme.value"
      @pointerdown="keys.onPointerDown"
      @keydown="keys.onKeyDown"
      @keyup="keys.release"
      @blur="keys.release"
      @change="onChange"
    >
      <option value="" disabled selected>{{ label }}</option>
      <option v-for="choice in choices" :key="choice.value" :value="choice.value">{{ choice.label }}</option>
    </select>
    <UIcon
      :name="THEME_ICON"
      class="pointer-events-none absolute top-1/2 left-2 size-4 -translate-y-1/2"
      :data-icon="THEME_ICON"
      data-testid="theme-toggle-icon"
    />
    <span
      class="pointer-events-none absolute right-0 bottom-0 flex size-3 items-center justify-center rounded-full bg-default ring ring-default"
    >
      <UIcon
        :name="THEME_MARKS[shell.theme.value]"
        class="size-2.5 text-muted"
        :data-icon="THEME_MARKS[shell.theme.value]"
        data-testid="theme-toggle-mark"
      />
    </span>
  </div>
</template>
