<script setup lang="ts">
// The control of one value, chosen from its JSON schema (`kinds.ts`): a switch, a number with
// its bounds, the string kinds (`ScalarControl`: text with suggestions, a path with its native
// dialog, a colour, a choice), a multi-select of allowed values (`MultiSelectControl`,
// RB-080-20), a list of strings (`ListControl`), a list or a map of objects as one-line entries
// (`CollapsibleList`, `MapControl`, RB-080-13), a pick list of values the core knows and a map
// of entry and choice (`PickListControl`, `ChoiceMapControl`, RB-080-72), an object's fields, or
// JSON for anything else - an unset value as an empty box, never the text `null`.
// Text and numbers are committed on change (Enter or leaving the field); the rest at once.
// Nested values (a breakpoint's width, an extension's folder) render through this component
// again, the path dialog handed down with them. The choices the core found for a dynamic
// field (`dynamic`, RB-080-32) belong to the key itself and go to its scalar control, pick list
// or choice map only.
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import USwitch from '@nuxt/ui/components/Switch.vue';
import UTextarea from '@nuxt/ui/components/Textarea.vue';
import type { JsonValue, PathKind } from '../ipc/protocol';
import { boundsOf, isNullable, itemsOf, kindOf, mapValuesOf, newItem, propertiesOf, sourceOf } from './kinds';
import ChoiceMapControl from './ChoiceMapControl.vue';
import CollapsibleList from './CollapsibleList.vue';
import ListControl from './ListControl.vue';
import { copyItem } from './listOps';
import MapControl from './MapControl.vue';
import MultiSelectControl from './MultiSelectControl.vue';
import PickListControl from './PickListControl.vue';
import ScalarControl from './ScalarControl.vue';
import type { DynamicData, JsonSchema } from './schema';
import { summaryOf } from './summary';

defineOptions({ name: 'FieldControl' });

const props = defineProps<{
  schema: JsonSchema;
  value: JsonValue;
  /** The key, a nested field's as its full path (`checks.lighthouse.categories`): its last
   * segment hints colours, the whole names the choices (`settings.option.<key>.<value>`). */
  name: string;
  /** The accessible name. */
  label: string;
  inputId: string;
  /** The id of the setting's description (RB-090-23); only the top-level control carries it. */
  describedBy?: string;
  disabled?: boolean;
  /** Opens the native dialog for a path; `null` when cancelled. */
  pickPath?: (kind: PathKind) => Promise<string | null>;
  /** The choices the core found for this key at runtime (RB-080-32). */
  dynamic?: DynamicData;
}>();
const emit = defineEmits<{ update: [value: JsonValue] }>();
const { t, te } = useI18n();

const kind = computed(() => kindOf(props.schema, props.name));
const nullable = computed(() => isNullable(props.schema));
const bounds = computed(() => boundsOf(props.schema));
const items = computed(() => itemsOf(props.schema));
const mapValues = computed(() => mapValuesOf(props.schema));
const fields = computed(() => propertiesOf(props.schema));
const list = computed<JsonValue[]>(() => (Array.isArray(props.value) ? props.value : []));
const object = computed<{ [key: string]: JsonValue }>(() =>
  typeof props.value === 'object' && props.value !== null && !Array.isArray(props.value)
    ? props.value
    : {},
);

const source = computed(() => sourceOf(props.schema));
/** The add line's label and the empty text: the source's own (`Add rule`), else the general. */
const pickTexts = computed(() => {
  const text = (part: 'add' | 'empty'): string => {
    const own = `settings.pick.source.${source.value ?? ''}.${part}`;
    return source.value !== null && te(own) ? t(own) : t(`settings.pick.${part}`);
  };
  return { add: text('add'), empty: text('empty') };
});

const jsonText = ref('');
const jsonError = ref(false);
watch(
  () => props.value,
  (value) => {
    jsonText.value = value === null ? '' : JSON.stringify(value, null, 2);
    jsonError.value = false;
  },
  { immediate: true },
);

const INPUT =
  'h-8 w-full rounded-md bg-default px-2 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-75';

/** The kinds `ScalarControl` renders. */
const SCALAR: ReadonlySet<string> = new Set(['string', 'path', 'color', 'enum']);
/** Nested values that take the full width of an object's two columns (lists, maps, objects). */
const WIDE: ReadonlySet<string> = new Set([
  'stringList',
  'objectList',
  'objectMap',
  'object',
  'json',
  'multiEnum',
  'pickList',
  'choiceMap',
]);

function fieldLabel(field: string): string {
  const key = `settings.field.${field}`;
  return te(key) ? t(key) : field;
}

function onNumber(event: Event): void {
  const raw = (event.target as HTMLInputElement).value.trim();
  if (raw === '') {
    if (nullable.value) {
      emit('update', null);
    }
    return;
  }
  const number = Number(raw);
  if (!Number.isNaN(number)) {
    emit('update', number);
  }
}

function onJson(): void {
  if (jsonText.value.trim() === '') {
    jsonError.value = false;
    if (props.value !== null && nullable.value) {
      emit('update', null);
    }
    return;
  }
  try {
    emit('update', JSON.parse(jsonText.value) as JsonValue);
    jsonError.value = false;
  } catch {
    jsonError.value = true;
  }
}

function onField(field: string, value: JsonValue): void {
  const next = { ...object.value };
  if (value === null) {
    delete next[field];
  } else {
    next[field] = value;
  }
  emit('update', next);
}
</script>

<template>
  <div class="min-w-0" :data-kind="kind">
    <USwitch
      v-if="kind === 'boolean'"
      :aria-describedby="describedBy"
      :id="inputId"
      :model-value="value === true"
      :disabled="disabled"
      :aria-label="label"
      @update:model-value="(on: boolean) => emit('update', on)"
    />

    <input
      v-else-if="kind === 'integer' || kind === 'number'"
      :aria-describedby="describedBy"
      :id="inputId"
      type="number"
      :class="INPUT"
      class="max-w-48"
      :value="typeof value === 'number' ? value : ''"
      :min="bounds.min"
      :max="bounds.max"
      :step="kind === 'integer' ? 1 : 'any'"
      :disabled="disabled"
      :aria-label="label"
      @change="onNumber"
    />

    <ScalarControl
      v-else-if="SCALAR.has(kind)"
      :described-by="describedBy"
      :schema="schema"
      :value="value"
      :kind="kind"
      :label="label"
      :input-id="inputId"
      :disabled="disabled"
      :pick-path="pickPath"
      :dynamic="dynamic"
      :name="name"
      @update="(next) => emit('update', next)"
    />

    <MultiSelectControl
      v-else-if="kind === 'multiEnum'"
      :aria-describedby="describedBy"
      :schema="schema"
      :value="list"
      :name="name"
      :label="label"
      :input-id="inputId"
      :disabled="disabled"
      @update="(next) => emit('update', next)"
    />

    <PickListControl
      v-else-if="kind === 'pickList'"
      :aria-describedby="describedBy"
      :value="list"
      :label="label"
      :input-id="inputId"
      :disabled="disabled"
      :dynamic="dynamic"
      :texts="pickTexts"
      @update="(next) => emit('update', next)"
    />

    <ChoiceMapControl
      v-else-if="kind === 'choiceMap'"
      :aria-describedby="describedBy"
      :schema="schema"
      :value="value"
      :name="name"
      :label="label"
      :input-id="inputId"
      :disabled="disabled"
      :dynamic="dynamic"
      :texts="pickTexts"
      @update="(next) => emit('update', next)"
    />

    <CollapsibleList
      v-else-if="kind === 'objectList'"
      :aria-describedby="describedBy"
      :items="list"
      :label="label"
      :disabled="disabled"
      :fresh="() => newItem(items)"
      :copy="copyItem"
      :summary="(item) => summaryOf(item, t)"
      @update="(next) => emit('update', next)"
    >
      <template #item="{ item, index, update }">
        <FieldControl
          :schema="items"
          :value="item"
          :name="name"
          :label="`${label} ${index + 1}`"
          :input-id="`${inputId}-${index}`"
          :disabled="disabled"
          :pick-path="pickPath"
          @update="update"
        />
      </template>
    </CollapsibleList>

    <MapControl
      v-else-if="kind === 'objectMap'"
      :aria-describedby="describedBy"
      :value="object"
      :label="label"
      :input-id="inputId"
      :disabled="disabled"
      :fresh="() => newItem(mapValues)"
      @update="(next) => emit('update', next)"
    >
      <template #value="{ value: entry, index, update }">
        <FieldControl
          :schema="mapValues"
          :value="entry"
          :name="name"
          :label="`${label} ${index + 1}`"
          :input-id="`${inputId}-${index}`"
          :disabled="disabled"
          :pick-path="pickPath"
          @update="update"
        />
      </template>
    </MapControl>

    <ListControl
      v-else-if="kind === 'stringList'"
      :aria-describedby="describedBy"
      :items="list"
      :label="label"
      :disabled="disabled"
      :fresh="() => newItem(items)"
      @update="(next) => emit('update', next)"
    >
      <template #item="{ item, index, update }">
        <FieldControl
          :schema="items"
          :value="item"
          :name="name"
          :label="`${label} ${index + 1}`"
          :input-id="`${inputId}-${index}`"
          :disabled="disabled"
          :pick-path="pickPath"
          @update="update"
        />
      </template>
    </ListControl>

    <div v-else-if="kind === 'object'" class="grid gap-2 sm:grid-cols-2">
      <div
        v-for="[field, fieldSchema] in fields"
        :key="field"
        class="flex min-w-0 flex-col gap-1"
        :class="WIDE.has(kindOf(fieldSchema, field)) ? 'sm:col-span-2' : ''"
      >
        <label :for="`${inputId}-${field}`" class="text-xs font-medium text-muted">
          {{ fieldLabel(field) }}
        </label>
        <FieldControl
          :schema="fieldSchema"
          :value="object[field] ?? null"
          :name="`${name}.${field}`"
          :label="`${label}: ${fieldLabel(field)}`"
          :input-id="`${inputId}-${field}`"
          :disabled="disabled"
          :pick-path="pickPath"
          @update="(next) => onField(field, next)"
        />
      </div>
    </div>

    <div v-else class="flex flex-col gap-1">
      <UTextarea
        :id="inputId"
        :aria-describedby="describedBy"
        v-model="jsonText"
        :rows="Math.min(12, jsonText.split('\n').length + 1)"
        :disabled="disabled"
        :aria-label="label"
        :aria-invalid="jsonError"
        class="w-full font-mono"
        autoresize
        @blur="onJson"
      />
      <p v-if="jsonError" class="text-xs text-error" role="alert">{{ t('settings.json.parse_error') }}</p>
    </div>
  </div>
</template>
