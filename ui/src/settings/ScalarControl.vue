<script setup lang="ts">
// One string value (RB-080-20): a path with Browse for its native dialog (`x-input`: file or
// folder), a colour, a choice of the allowed values, or free text, with the values `x-options`
// suggests offered while typing. Committed on change (Enter or leaving the field); the choice
// at once. `FieldControl` picks this component for the string kinds. A path whose choices the
// core found (`dynamic`, RB-080-32: the installed Firefox editions) offers them labelled, shows
// the first as the placeholder of an empty field ("Found: ..."), "Not found" when there is none,
// and marks a configured path that names no file any more.
// Nuxt UI throughout (RB-080-88): `UInput`, `USelect`, and `SuggestInput` (a `UInputMenu` with free
// entry) where values are proposed; a colour is `ColorInput` (`UColorPicker` in a `UPopover` and
// the hex field, RB-090-58).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import type { JsonValue, PathKind } from '../ipc/protocol';
import { allowsAlpha } from './colorValue';
import ColorInput from './ColorInput.vue';
import { SETTINGS_ICONS } from './icons';
import { baseSchema, enumOptions, type FieldKind, isNullable, optionsOf, pathInputOf } from './kinds';
import { useOptionName } from './optionNames';
import type { DynamicData, JsonSchema } from './schema';
import SuggestInput, { type Suggestion } from './SuggestInput.vue';

const props = defineProps<{
  schema: JsonSchema;
  value: JsonValue;
  kind: FieldKind;
  /** The accessible name. */
  label: string;
  inputId: string;
  /** The id of the setting's description (RB-090-23). */
  describedBy?: string;
  disabled?: boolean;
  /** Opens the native dialog for a path; `null` when cancelled. */
  pickPath?: (kind: PathKind) => Promise<string | null>;
  /** The choices the core found for this path at runtime. */
  dynamic?: DynamicData;
  /** The key: a choice's name is the catalogue's where it has one (`optionNames.ts`, RB-080-72,
   * RB-090-16); else the value itself. */
  name?: string;
}>();
const emit = defineEmits<{ update: [value: JsonValue] }>();
const { t } = useI18n();
const optionName = useOptionName();

const nullable = computed(() => isNullable(props.schema));
const options = computed(() => enumOptions(props.schema) ?? []);
const suggestions = computed<Suggestion[]>(() => optionsOf(props.schema).map((value) => ({ value, label: value })));
const pathKind = computed(() => pathInputOf(props.schema) ?? 'file');
const alpha = computed(() => allowsAlpha(baseSchema(props.schema)));
const text = computed(() => (typeof props.value === 'string' ? props.value : ''));
const selected = computed(() =>
  options.value.findIndex((option) => JSON.stringify(option.value) === JSON.stringify(props.value)),
);
/** The choices of an enum by index ("-1" unset): a select item's value is a string, never empty. */
const choiceItems = computed(() => [
  ...(nullable.value || selected.value < 0 ? [{ label: t('settings.value.unset'), value: '-1' }] : []),
  ...options.value.map((option, index) => ({ label: optionName(props.name, option.value), value: String(index) })),
]);
const found = computed<Suggestion[]>(() => props.dynamic?.options ?? []);
// The core trims the configured path before it looks for the file; the field holds it as typed.
const missing = computed(() => props.dynamic?.missing !== undefined && props.dynamic.missing === text.value.trim());
/** What an empty dynamic path means: the first installation, or that none was found. */
const placeholder = computed(() => {
  if (props.dynamic === undefined || text.value !== '') {
    return undefined;
  }
  const [first] = found.value;
  return first === undefined ? t('settings.path.not_found') : t('settings.path.found', { path: first.value });
});

function commitText(raw: string): void {
  emit('update', raw === '' && nullable.value ? null : raw);
}

function onText(event: Event): void {
  commitText((event.target as HTMLInputElement).value);
}

function onSelect(value: unknown): void {
  const index = Number(value);
  emit('update', index < 0 ? null : (options.value[index]?.value ?? null));
}

async function browse(): Promise<void> {
  const path = await props.pickPath?.(pathKind.value);
  if (path !== null && path !== undefined) {
    emit('update', path);
  }
}
</script>

<template>
  <div v-if="kind === 'path'" class="flex flex-col gap-1">
    <div class="flex items-center gap-2">
      <SuggestInput
        v-if="found.length > 0"
        :value="text"
        :items="found"
        :input-id="inputId"
        :label="label"
        :described-by="describedBy"
        :disabled="disabled"
        :placeholder="placeholder"
        :invalid="missing"
        @commit="commitText"
      />
      <UInput
        v-else
        :id="inputId"
        :aria-describedby="describedBy"
        :model-value="text"
        :disabled="disabled"
        :aria-label="label"
        :placeholder="placeholder"
        :aria-invalid="missing ? 'true' : undefined"
        spellcheck="false"
        class="w-full"
        @change="onText"
      />
      <UButton
        :icon="pathKind === 'folder' ? SETTINGS_ICONS.browseFolder : SETTINGS_ICONS.browse"
        color="neutral"
        variant="soft"
        :label="t('settings.path.browse')"
        :disabled="disabled || pickPath === undefined"
        data-testid="browse"
        :data-path-kind="pathKind"
        @click="browse"
      />
    </div>
    <p v-if="missing" class="text-xs text-error" role="alert" data-testid="path-missing">
      {{ t('settings.path.missing') }}
    </p>
  </div>

  <ColorInput
    v-else-if="kind === 'color'"
    :value="text"
    :label="label"
    :input-id="inputId"
    :described-by="describedBy"
    :disabled="disabled"
    :alpha="alpha"
    @update="commitText"
  />

  <USelect
    v-else-if="kind === 'enum'"
    :id="inputId"
    :aria-describedby="describedBy"
    :items="choiceItems"
    :model-value="String(selected)"
    :disabled="disabled"
    :aria-label="label"
    class="w-full max-w-64"
    @update:model-value="onSelect"
  />

  <SuggestInput
    v-else-if="suggestions.length > 0"
    :value="text"
    :items="suggestions"
    :input-id="inputId"
    :label="label"
    :described-by="describedBy"
    :disabled="disabled"
    @commit="commitText"
  />

  <UInput
    v-else
    :id="inputId"
    :aria-describedby="describedBy"
    :model-value="text"
    :disabled="disabled"
    :aria-label="label"
    class="w-full"
    @change="onText"
  />
</template>
