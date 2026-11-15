<script setup lang="ts">
// A list whose items are a closed choice (`enum` on the items, RB-080-20): one checkbox per
// allowed value, in the schema's order (the Lighthouse categories), named by the catalogue
// (`optionNames.ts`, the value in the tooltip) where it has a name. A value the
// schema does not know stays in the list, shown as its own box, so an older config loses nothing,
// with "Remove invalid value" beside it (RB-080-72).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { JsonValue } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';
import { enumOptions, itemsOf } from './kinds';
import { useOptionName } from './optionNames';
import type { JsonSchema } from './schema';

const props = defineProps<{
  schema: JsonSchema;
  value: JsonValue[];
  /** The key: names the values (`optionNames.ts`). */
  name?: string;
  /** The accessible name. */
  label: string;
  inputId: string;
  disabled?: boolean;
}>();
const emit = defineEmits<{ update: [value: JsonValue[]] }>();
const { t } = useI18n();
const optionName = useOptionName();

const known = computed(() => (enumOptions(itemsOf(props.schema)) ?? []).map((option) => option.value));
const choices = computed(() => {
  const extra = props.value.filter((item) => !known.value.some((option) => same(option, item)));
  return [...known.value, ...extra];
});

/** A value the schema knows reads as itself; any other as its JSON, marked, so an empty string or a category Lighthouse dropped is visible and can be unticked. */
function caption(option: JsonValue): string {
  if (!isKnown(option)) {
    return t('settings.multi_select.unknown', { value: JSON.stringify(option) });
  }
  return optionName(props.name, option);
}

function isKnown(option: JsonValue): boolean {
  return known.value.some((item) => same(item, option));
}

function removeInvalid(option: JsonValue): void {
  emit(
    'update',
    props.value.filter((item) => !same(item, option)),
  );
}

function same(a: JsonValue, b: JsonValue): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function checked(option: JsonValue): boolean {
  return props.value.some((item) => same(item, option));
}

function toggle(option: JsonValue, on: boolean): void {
  const without = props.value.filter((item) => !same(item, option));
  // Keep the schema's order: the chosen values in the order of the choices.
  const next = on ? choices.value.filter((choice) => same(choice, option) || without.some((item) => same(item, choice))) : without;
  emit('update', next);
}
</script>

<template>
  <fieldset class="flex flex-wrap gap-x-4 gap-y-1" :disabled="disabled" data-testid="multi-select">
    <legend class="sr-only">{{ label }}</legend>
    <span v-for="(option, index) in choices" :key="index" class="flex items-center gap-1.5">
    <label class="flex cursor-pointer items-center gap-1.5 text-sm" :class="isKnown(option) ? '' : 'text-warning'" :title="String(option)">
      <input
        :id="`${inputId}-${index}`"
        type="checkbox"
        class="accent-(--ui-primary) focus-visible:outline-2 focus-visible:outline-primary"
        :checked="checked(option)"
        :disabled="disabled"
        :value="String(option)"
        @change="(event) => toggle(option, (event.target as HTMLInputElement).checked)"
      />
      {{ caption(option) }}
    </label>
    <UButton
      v-if="!isKnown(option)"
      :icon="SETTINGS_ICONS.remove"
      color="warning"
      variant="ghost"
      size="xs"
      :label="t('settings.multi_select.remove_invalid')"
      :disabled="disabled"
      data-testid="remove-invalid"
      @click="removeInvalid(option)"
    />
    </span>
  </fieldset>
</template>
