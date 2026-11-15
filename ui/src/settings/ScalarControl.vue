<script setup lang="ts">
// One string value (RB-080-20): a path with Browse for its native dialog (`x-input`: file or
// folder), a colour, a choice of the allowed values, or free text, with the values `x-options`
// suggests offered while typing. Committed on change (Enter or leaving the field); the choice
// at once. `FieldControl` picks this component for the string kinds. A path whose choices the
// core found (`dynamic`, RB-080-32: the installed Firefox editions) offers them labelled, shows
// the first as the placeholder of an empty field ("Found: ..."), "Not found" when there is none,
// and marks a configured path that names no file any more.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { JsonValue, PathKind } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';
import { enumOptions, type FieldKind, isNullable, optionsOf, pathInputOf } from './kinds';
import { useOptionName } from './optionNames';
import type { DynamicData, JsonSchema } from './schema';

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
const suggestions = computed(() => optionsOf(props.schema));
const pathKind = computed(() => pathInputOf(props.schema) ?? 'file');
const text = computed(() => (typeof props.value === 'string' ? props.value : ''));
const selected = computed(() =>
  options.value.findIndex((option) => JSON.stringify(option.value) === JSON.stringify(props.value)),
);
const listId = computed(() => `${props.inputId}-options`);
const found = computed(() => props.dynamic?.options ?? []);
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

const INPUT =
  'h-8 w-full rounded-md bg-default px-2 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-75';

function onText(event: Event): void {
  const raw = (event.target as HTMLInputElement).value;
  emit('update', raw === '' && nullable.value ? null : raw);
}

function onSelect(event: Event): void {
  const index = Number((event.target as HTMLSelectElement).value);
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
      <input
        :id="inputId"
        :aria-describedby="describedBy"
        type="text"
        :class="INPUT"
        :value="text"
        :disabled="disabled"
        :aria-label="label"
        :placeholder="placeholder"
        :list="found.length > 0 ? listId : undefined"
        :aria-invalid="missing ? 'true' : undefined"
        spellcheck="false"
        @change="onText"
      />
      <UButton
        :icon="pathKind === 'folder' ? SETTINGS_ICONS.browseFolder : SETTINGS_ICONS.browse"
        color="neutral"
        variant="soft"
        size="sm"
        :label="t('settings.path.browse')"
        :disabled="disabled || pickPath === undefined"
        data-testid="browse"
        :data-path-kind="pathKind"
        @click="browse"
      />
    </div>
    <datalist v-if="found.length > 0" :id="listId" data-testid="suggestions">
      <option v-for="option in found" :key="option.value" :value="option.value">{{ option.label }}</option>
    </datalist>
    <p v-if="missing" class="text-xs text-error" role="alert" data-testid="path-missing">
      {{ t('settings.path.missing') }}
    </p>
  </div>

  <div v-else-if="kind === 'color'" class="flex items-center gap-2">
    <input
      type="color"
      class="h-8 w-10 rounded-md bg-default ring ring-inset ring-accented"
      :value="/^#[0-9a-fA-F]{6}$/.test(text) ? text : '#000000'"
      :disabled="disabled"
      :aria-label="label"
      @change="onText"
    />
    <input
      :id="inputId"
      :aria-describedby="describedBy"
      type="text"
      :class="INPUT"
      class="max-w-32 font-mono"
      :value="text"
      :disabled="disabled"
      :aria-label="label"
      @change="onText"
    />
  </div>

  <select
    v-else-if="kind === 'enum'"
    :id="inputId"
    :aria-describedby="describedBy"
    :class="INPUT"
    class="max-w-64"
    :value="String(selected)"
    :disabled="disabled"
    :aria-label="label"
    @change="onSelect"
  >
    <option v-if="nullable || selected < 0" value="-1">{{ t('settings.value.unset') }}</option>
    <option v-for="(option, index) in options" :key="index" :value="String(index)">
      {{ optionName(name, option.value) }}
    </option>
  </select>

  <div v-else>
    <input
      :id="inputId"
      :aria-describedby="describedBy"
      type="text"
      :class="INPUT"
      :value="text"
      :disabled="disabled"
      :aria-label="label"
      :list="suggestions.length > 0 ? listId : undefined"
      @change="onText"
    />
    <datalist v-if="suggestions.length > 0" :id="listId" data-testid="suggestions">
      <option v-for="option in suggestions" :key="option" :value="option" />
    </datalist>
  </div>
</template>
