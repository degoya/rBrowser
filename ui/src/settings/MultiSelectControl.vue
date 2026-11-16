<script setup lang="ts">
// A list whose items are a closed choice (`enum` on the items, RB-080-20): one checkbox per
// allowed value, in the schema's order (the Lighthouse categories), named by the catalogue
// (`optionNames.ts`, the value in the tooltip) where it has a name. A value the
// schema does not know stays in the list, shown as its own box, so an older config loses nothing,
// with "Remove invalid value" beside it (RB-080-72). A Nuxt UI `UCheckboxGroup` (RB-080-88); a box
// is keyed by its place among the choices, so an empty string or a number is a box of its own.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UCheckboxGroup from '@nuxt/ui/components/CheckboxGroup.vue';
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

/** One box per choice, keyed by its index (a key goes into an element id, so no JSON quotes);
 * the choice itself rides along for the slots. */
const items = computed(() =>
  choices.value.map((option, index) => ({ value: String(index), label: caption(option), option })),
);
const checked = computed(() => items.value.filter((item) => props.value.some((v) => same(v, item.option))).map((item) => item.value));

/** The ticked boxes, in the schema's order: the chosen values in the order of the choices. */
function choose(keys: unknown): void {
  const ticked = new Set(Array.isArray(keys) ? keys.map(String) : []);
  emit(
    'update',
    items.value.filter((item) => ticked.has(item.value)).map((item) => item.option),
  );
}
</script>

<template>
  <UCheckboxGroup
    :model-value="checked"
    :items="items"
    :legend="label"
    orientation="horizontal"
    :disabled="disabled"
    :ui="{ legend: 'sr-only', fieldset: 'flex-wrap gap-x-4 gap-y-1', description: 'empty:hidden' }"
    data-testid="multi-select"
    @update:model-value="choose"
  >
    <template #label="{ item }">
      <span :class="isKnown(item.option) ? '' : 'text-warning'" :title="String(item.option)">{{ item.label }}</span>
    </template>
    <template #description="{ item }">
      <UButton
        v-if="!isKnown(item.option)"
        :icon="SETTINGS_ICONS.remove"
        color="warning"
        variant="ghost"
        size="xs"
        :label="t('settings.multi_select.remove_invalid')"
        :disabled="disabled"
        data-testid="remove-invalid"
        @click="removeInvalid(item.option)"
      />
    </template>
  </UCheckboxGroup>
</template>
