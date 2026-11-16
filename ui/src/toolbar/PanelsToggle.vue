<script setup lang="ts">
// The plugin panels' switch at the right end of the toolbar (RB-080-79): with the panels docked as
// their icon rail from the start (RB-080-93) it opens them at their size - or their own window -
// and folds them back to the rail, with the same command as `Leader P Z`, the rail's expand
// button and the palette. A switch (`role="switch"`), on while the panels are open; its icon is
// the place they dock at. A panel a command or a check opened while they are folded puts a chip
// on it until they open (the rail marks the entry, RB-080-61). Its tooltip names the command's
// shortcut (as `FooterToggle.vue`). Off by default since RB-090-10 - the icon rail beside the
// views opens the panels - so nothing is drawn until `ui.toolbar.panelsToggle` is on. In a narrow
// window its group is the last to leave the row for the overflow menu (`groups.ts`).
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UChip from '@nuxt/ui/components/Chip.vue';
import { DOCK_ICONS } from '../panels/icons';
import { booleanSetting } from '../settings/booleanSetting';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { PANELS_COMMAND, PANELS_DOCK_SETTING, PANELS_STATE_SETTING, PANELS_TOGGLE_SETTING } from './panelsToggle';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const shown = computed(() => booleanSetting(shell.settings.value[PANELS_TOGGLE_SETTING], false));
/** The core's default is the rail: only `open` turns the switch on. */
const open = computed(() => shell.settings.value[PANELS_STATE_SETTING] === 'open');
const icon = computed(() => {
  const place = shell.settings.value[PANELS_DOCK_SETTING];
  return place === 'left' || place === 'bottom' || place === 'floating'
    ? DOCK_ICONS.position[place]
    : DOCK_ICONS.position.right;
});

/** A panel opened by a command while the panels are not open. */
const attention = ref(false);
watch(open, (now) => {
  if (now) {
    attention.value = false;
  }
});
const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'pluginPanel' && 'select' in message.message && !open.value) {
    attention.value = true;
  }
});
onBeforeUnmount(stop);

const label = computed(() => t(attention.value ? 'shell.toolbar.panels_attention' : 'shell.toolbar.panels'));
const title = computed(() => {
  const shortcut = shell.bindingOf(PANELS_COMMAND);
  return shortcut === null ? label.value : t('shell.toolbar.panels_shortcut', { label: label.value, shortcut });
});

onMounted(() => shell.loadCommands());
</script>

<template>
  <UChip v-if="shown" :show="attention" color="primary" inset size="md">
    <UButton
      :icon="icon"
      color="neutral"
      :variant="open ? 'soft' : 'ghost'"
      size="sm"
      square
      role="switch"
      :aria-label="label"
      :aria-checked="open ? 'true' : 'false'"
      :title="title"
      :data-icon="icon"
      :data-attention="attention ? 'true' : undefined"
      data-testid="panels-toggle"
      @click="shell.run({ command: PANELS_COMMAND })"
    />
  </UChip>
</template>
