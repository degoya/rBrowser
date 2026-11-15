<script setup lang="ts">
// The environment switcher (RB-010-12): a native select of `environments[]` with a swatch of the
// current environment's frame colour. Switching keeps path, query and hash (the core).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useShell } from '../state/shell';
import NativeSelect, { type SelectOption } from './NativeSelect.vue';

const shell = useShell();
const { t } = useI18n();

const COLOR = /^#[0-9a-fA-F]{6}$/;

const environments = computed(() => shell.state.value?.environments ?? []);
const current = computed(() => shell.state.value?.environment ?? null);
const options = computed<SelectOption[]>(() =>
  environments.value.map((e) => ({ value: e.id, label: e.id })),
);
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
    <NativeSelect
      :label="t('shell.toolbar.environment')"
      :empty-label="t('shell.toolbar.environment_none')"
      :none-label="t('shell.toolbar.environment_unset')"
      :options="options"
      :model-value="current"
      @update:model-value="(id) => shell.run({ command: 'environments.select', id })"
    />
  </div>
</template>
