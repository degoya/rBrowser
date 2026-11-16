<script setup lang="ts">
// A map of named objects - the layout presets (`layouts.presets`) - as the same one-line entries
// as a list (RB-080-13): the name and a summary closed, the name and the fields open. The order is
// the map's (`leader L 1...9` follows it), so moving an entry rewrites the map in the new order.
// A name must be set and unique; a refused rename keeps the old one and says why. The name is a
// Nuxt UI `UInput`, committed on change (RB-080-88).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UInput from '@nuxt/ui/components/Input.vue';
import type { JsonValue } from '../ipc/protocol';
import CollapsibleList from './CollapsibleList.vue';
import { uniqueName } from './listOps';
import { entrySummary } from './summary';

type Entry = [string, JsonValue];

const props = defineProps<{
  value: { [name: string]: JsonValue };
  label: string;
  inputId: string;
  disabled?: boolean;
  /** The value a new entry starts with. */
  fresh: () => JsonValue;
}>();
const emit = defineEmits<{ update: [value: { [name: string]: JsonValue }] }>();
const { t } = useI18n();
const refused = ref<number | null>(null);

const entries = computed<JsonValue[]>(() => Object.entries(props.value).map(([name, value]) => [name, value]));
const names = computed(() => new Set(Object.keys(props.value)));

function entryOf(item: JsonValue): Entry {
  return Array.isArray(item) && typeof item[0] === 'string' ? [item[0], item[1] ?? null] : ['', item];
}

function commit(items: JsonValue[]): void {
  emit('update', Object.fromEntries(items.map(entryOf)));
}

function fresh(): JsonValue {
  return [uniqueName('new', names.value), props.fresh()];
}

function copy(item: JsonValue): JsonValue {
  const [name, value] = entryOf(item);
  return [uniqueName(`${name}-copy`, names.value), JSON.parse(JSON.stringify(value)) as JsonValue];
}

function summary(item: JsonValue): string {
  const [name, value] = entryOf(item);
  return entrySummary(name, value, t);
}

function rename(index: number, event: Event, update: (item: JsonValue) => void): void {
  const input = event.target as HTMLInputElement;
  const [name, value] = entryOf(entries.value[index] ?? null);
  const next = input.value.trim();
  if (next === name) {
    refused.value = null;
    return;
  }
  if (next === '' || names.value.has(next)) {
    refused.value = index;
    input.value = name;
    return;
  }
  refused.value = null;
  update([next, value]);
}
</script>

<template>
  <CollapsibleList
    :items="entries"
    :label="label"
    :disabled="disabled"
    :fresh="fresh"
    :copy="copy"
    :summary="summary"
    @update="commit"
  >
    <template #item="{ item, index, update }">
      <div class="flex flex-col gap-1">
        <label :for="`${inputId}-${index}-name`" class="text-xs font-medium text-muted">
          {{ t('settings.map.name') }}
        </label>
        <UInput
          :id="`${inputId}-${index}-name`"
          :model-value="entryOf(item)[0]"
          :disabled="disabled"
          :aria-invalid="refused === index"
          spellcheck="false"
          class="w-full max-w-64"
          data-testid="map-name"
          @change="rename(index, $event, update)"
        />
        <p v-if="refused === index" class="text-xs text-error" role="alert" data-testid="map-name-refused">
          {{ t('settings.map.name_invalid') }}
        </p>
      </div>
      <slot
        name="value"
        :value="entryOf(item)[1]"
        :index="index"
        :update="(value: JsonValue) => update([entryOf(item)[0], value])"
      />
    </template>
  </CollapsibleList>
</template>
