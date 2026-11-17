<script setup lang="ts">
// The environment switcher (RB-010-12): a pulldown of `environments[]` (the core's native menu,
// RB-090-45) with a swatch of the current environment's frame colour. Switching keeps path, query
// and hash (the core).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useShell } from '../state/shell';
import PulldownButton from './PulldownButton.vue';
import { choiceMenu, shownChoice, type Choice } from './pulldown';

const shell = useShell();
const { t } = useI18n();

const COLOR = /^#[0-9a-fA-F]{6}$/;

const environments = computed(() => shell.state.value?.environments ?? []);
const current = computed(() => shell.state.value?.environment ?? null);
const choices = computed<Choice[]>(() => environments.value.map((e) => ({ value: e.id, label: e.id })));
const shown = computed(() =>
  shownChoice(
    choices.value,
    current.value,
    t('shell.toolbar.environment_unset'),
    t('shell.toolbar.environment_none'),
  ),
);
/** The rows carry their place; a row chosen maps back to its environment (RB-090-57). */
const menu = choiceMenu();
const rows = () => menu.rows(choices.value, current.value);

function choose(id: string): void {
  const value = menu.valueOf(id);
  if (value !== undefined) {
    shell.run({ command: 'environments.select', id: value });
  }
}
/** The colour of the current environment, only in the form the core frames with. */
const color = computed(() => {
  const value = environments.value.find((e) => e.id === current.value)?.color;
  return value !== undefined && COLOR.test(value) ? value : null;
});
</script>

<template>
  <div class="flex shrink-0 items-center gap-1" data-testid="environment">
    <span
      v-if="color !== null"
      role="img"
      :aria-label="`${t('shell.toolbar.environment_color')} ${color}`"
      :title="color"
      class="size-3 shrink-0 rounded-full ring ring-inset ring-accented"
      :style="{ backgroundColor: color }"
      data-testid="environment-color"
    />
    <PulldownButton
      kind="environment"
      :label="t('shell.toolbar.environment')"
      :value="shown"
      :rows="rows"
      :disabled="choices.length === 0"
      @choose="choose"
    />
  </div>
</template>
