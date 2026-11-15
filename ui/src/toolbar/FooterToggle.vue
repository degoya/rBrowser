<script setup lang="ts">
// The breakpoint bar's switch in the toolbar (RB-080-60; the strip was called the footer until
// RB-090-11, its keys and ids still say so): shows or hides the breakpoint bar under every view
// with the same command as `Leader O B`, the view menu and the page's context menu - a switch
// (`role="switch"`), on while the bar shows, so the overflow menu names both states. Its
// tooltip names the command's shortcut, from the bindings the shell keeps current (asked for
// once, then the core's `commands` events), so the text is ready before a hover and follows an
// edit. `ui.toolbar.footerToggle` off: nothing is drawn. In a narrow window its group leaves the
// row; the overflow menu's entry clicks it (`groups.ts`).
import { computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import { booleanSetting } from '../settings/booleanSetting';
import { useShell } from '../state/shell';
import { FOOTER_COMMAND, FOOTER_TOGGLE_SETTING, FOOTER_VISIBLE_SETTING } from './footerToggle';

const shell = useShell();
const { t } = useI18n();

const shown = computed(() => booleanSetting(shell.settings.value[FOOTER_TOGGLE_SETTING], true));
const visible = computed(() => booleanSetting(shell.settings.value[FOOTER_VISIBLE_SETTING], true));
const label = computed(() => t('shell.toolbar.footer'));
const title = computed(() => {
  const shortcut = shell.bindingOf(FOOTER_COMMAND);
  return shortcut === null ? label.value : t('shell.toolbar.footer_shortcut', { label: label.value, shortcut });
});

onMounted(() => shell.loadCommands());
</script>

<template>
  <UButton
    v-if="shown"
    :icon="ICONS.footer"
    color="neutral"
    :variant="visible ? 'soft' : 'ghost'"
    size="sm"
    square
    role="switch"
    :aria-label="label"
    :aria-checked="visible ? 'true' : 'false'"
    :title="title"
    data-testid="footer-toggle"
    @click="shell.run({ command: FOOTER_COMMAND })"
  />
</template>
