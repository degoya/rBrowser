<script setup lang="ts">
// The choice of rBrowser's own theme in the toolbar (RB-080-95, RB-090-10): System, Light, Dark
// for the shell, the panels, the settings and every window, the native parts and the title bar
// included - never the pages in the views, whose colour scheme is the emulation's sun and moon.
// One icon in every state, a sun and a moon in one, so it reads as "light or dark" at a glance;
// a small mark on it (a screen, a sun with rays, a moon with stars) says the theme in effect,
// and its name and tooltip say it in words - for System with what Windows uses now.
//
// The three open as the core's native menu below the button (RB-090-41, `nativeMenu.ts`), the
// one in effect checked, in rBrowser's theme like the canvas zoom's presets - a `<select>`'s
// list was Chromium's popup, with other colours under the mouse and standing out of the window.
// A click, Enter, Space or an arrow key opens it, Esc closes it, the focus comes back to the
// button; a choice runs `ui.theme.set`. The tooltip names the shortcut that steps through the
// three (`ui.theme.cycle`, `Leader V 0`), from the bindings the shell keeps current.
// `ui.toolbar.themeToggle` off: nothing is drawn. In a narrow window its group leaves the row
// and the overflow menu shows the three choices under its name (`provideMenuSource`).
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import type { NativeMenuRow } from '../ipc/protocol';
import { booleanSetting } from '../settings/booleanSetting';
import { useShell } from '../state/shell';
import { parseTheme } from '../theme/theme';
import { isOpeningKey, provideMenuSource, useNativeMenu } from './nativeMenu';
import {
  THEME_CHOICES,
  THEME_CYCLE_COMMAND,
  THEME_ICON,
  THEME_MARKS,
  THEME_SET_COMMAND,
  THEME_TOGGLE_SETTING,
  themeChoiceKey,
  themeLabelKey,
} from './themeToggle';

const shell = useShell();
const { t } = useI18n();
const menu = useNativeMenu('theme', { send: shell.send });

const anchor = ref<HTMLElement | null>(null);
const shown = computed(() => booleanSetting(shell.settings.value[THEME_TOGGLE_SETTING], true));
const label = computed(() => t(themeLabelKey(shell.theme.value, shell.systemTheme.value)));
const title = computed(() => {
  const shortcut = shell.bindingOf(THEME_CYCLE_COMMAND);
  return shortcut === null
    ? t('shell.toolbar.theme.hint', { label: label.value })
    : t('shell.toolbar.theme.hint_shortcut', { label: label.value, shortcut });
});

/** System (with what Windows uses now), Light, Dark; the one in effect checked. */
function rows(): NativeMenuRow[] {
  return THEME_CHOICES.map((theme): NativeMenuRow => ({
    kind: 'item',
    id: theme,
    label: t(themeChoiceKey(theme, shell.systemTheme.value)),
    checked: theme === shell.theme.value,
  }));
}

function choose(id: string): void {
  void shell.run({ command: THEME_SET_COMMAND, theme: parseTheme(id) });
}

function open(): void {
  if (anchor.value !== null) {
    void menu.open(anchor.value, rows(), choose);
  }
}

function onKeyDown(event: KeyboardEvent): void {
  if (isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
}

/** The button's rows for the overflow menu, while the button is there. */
let stopSource: (() => void) | null = null;
watch(
  anchor,
  (element) => {
    stopSource?.();
    const button = element?.querySelector('button');
    stopSource = button ? provideMenuSource(button, { rows, choose }) : null;
  },
  { flush: 'post' },
);
onBeforeUnmount(() => stopSource?.());

onMounted(() => shell.loadCommands());
</script>

<template>
  <span v-if="shown" ref="anchor" class="relative inline-flex shrink-0 items-center">
    <UButton
      color="neutral"
      variant="ghost"
      size="sm"
      square
      class="size-8 justify-center"
      aria-haspopup="menu"
      :aria-label="label"
      :title="title"
      data-testid="theme-toggle"
      :data-theme-setting="shell.theme.value"
      @click="open"
      @keydown="onKeyDown"
    >
      <UIcon :name="THEME_ICON" class="size-4" :data-icon="THEME_ICON" data-testid="theme-toggle-icon" />
    </UButton>
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
  </span>
</template>
