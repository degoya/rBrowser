<script setup lang="ts">
// A map from a name to a closed choice - the rule overrides `checks.rules` (RB-080-72): one line
// per entry, its name (offered by the core, `x-options: "check-rules"`, with the id beside it) and
// the choice (off or a severity), a remove button, and a line that adds an entry. A new entry
// starts at the first choice ("Off"). Unset or empty, the field says so - never a raw `null`.
// The choice is a Nuxt UI `USelect` (RB-080-88), its items keyed by index.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import type { JsonValue } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';
import { enumOptions, mapValuesOf } from './kinds';
import { useOptionName } from './optionNames';
import PickAdd from './PickAdd.vue';
import { isMissing, nameOf, picksOf } from './picks';
import type { DynamicData, JsonSchema } from './schema';

const props = defineProps<{
  schema: JsonSchema;
  value: JsonValue;
  /** The key: names the choices (`optionNames.ts`). */
  name: string;
  /** The accessible name. */
  label: string;
  inputId: string;
  disabled?: boolean;
  dynamic?: DynamicData;
  texts: { add: string; empty: string };
}>();
const emit = defineEmits<{ update: [value: { [name: string]: JsonValue }] }>();
const { t } = useI18n();
const optionName = useOptionName();

const picks = computed(() => picksOf(props.dynamic));
const entries = computed<[string, JsonValue][]>(() =>
  typeof props.value === 'object' && props.value !== null && !Array.isArray(props.value)
    ? Object.entries(props.value)
    : [],
);
const names = computed(() => entries.value.map(([name]) => name));
const choices = computed(() => enumOptions(mapValuesOf(props.schema)) ?? []);
const choiceItems = computed(() =>
  choices.value.map((choice, index) => ({ label: optionName(props.name, choice.value), value: String(index) })),
);

function selectedIndex(value: JsonValue): number {
  return choices.value.findIndex((choice) => JSON.stringify(choice.value) === JSON.stringify(value));
}

function write(next: [string, JsonValue][]): void {
  emit('update', Object.fromEntries(next));
}

function change(index: number, picked: unknown): void {
  const choice = choices.value[Number(picked)];
  if (choice !== undefined) {
    write(entries.value.map(([name, value], i) => [name, i === index ? choice.value : value]));
  }
}

function add(name: string): void {
  const first = choices.value[0];
  if (first !== undefined) {
    write([...entries.value, [name, first.value]]);
  }
}

function remove(index: number): void {
  write(entries.value.filter((_, i) => i !== index));
}
</script>

<template>
  <div class="flex flex-col gap-2" role="group" :aria-label="label" data-testid="choice-map">
    <p v-if="entries.length === 0" class="text-sm text-muted" data-testid="pick-empty">{{ texts.empty }}</p>
    <ul v-else class="flex flex-col gap-1">
      <li
        v-for="([entry, choice], index) in entries"
        :key="entry"
        class="flex flex-wrap items-center gap-2 rounded-md border border-default px-2 py-1"
        data-testid="pick-item"
      >
        <span class="min-w-0 flex-1 truncate text-sm" :title="entry">
          <span data-testid="pick-name">{{ nameOf(picks, entry) }}</span>
          <span v-if="nameOf(picks, entry) !== entry" class="ml-2 font-mono text-xs text-muted" data-testid="pick-id">{{ entry }}</span>
        </span>
        <UBadge
          v-if="isMissing(picks, entry, dynamic !== undefined && !picks.open)"
          color="warning"
          variant="subtle"
          size="sm"
          :label="t('settings.pick.missing')"
          data-testid="pick-missing"
        />
        <USelect
          :id="`${inputId}-${index}`"
          :items="choiceItems"
          :model-value="String(selectedIndex(choice))"
          :disabled="disabled"
          :aria-label="t('settings.pick.choice', { name: nameOf(picks, entry) })"
          class="w-36"
          data-testid="pick-choice"
          @update:model-value="(picked: unknown) => change(index, picked)"
        />
        <UButton
          :icon="SETTINGS_ICONS.remove"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :disabled="disabled"
          :aria-label="t('settings.pick.remove', { name: nameOf(picks, entry) })"
          :title="t('settings.pick.remove', { name: nameOf(picks, entry) })"
          data-testid="pick-remove"
          @click="remove(index)"
        />
      </li>
    </ul>
    <PickAdd :picks="picks" :chosen="names" :input-id="inputId" :add-label="texts.add" :disabled="disabled" @add="add" />
  </div>
</template>
