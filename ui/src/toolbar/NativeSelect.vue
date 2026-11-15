<script setup lang="ts">
// A native <select>: CEF opens its list as a native popup that may cover the browser views and
// leaves nothing behind when it closes. An HTML listbox would be clipped by the shell view.
//
// The select's own value is always its first option, a disabled placeholder that names the
// option in effect (`modelValue`), `noneLabel` without one or `emptyLabel` without options. A
// native select fires `change` only when its value changes, so with the option in effect
// selected it could not be chosen again - the first preset, or the only one, did nothing
// (RB-080-16). Choosing any entry now changes the value, the parent gets it, and the select
// goes back to its placeholder.
//
// An arrow key or a typed letter on the closed select browses, it does not choose: such a key
// opens the list instead, and a `change` it causes all the same chooses nothing (`menuKeys.ts`,
// RB-090-23).
import { computed, useId } from 'vue';
import { createMenuKeys } from './menuKeys';

export interface SelectOption {
  value: string;
  label: string;
}

const props = defineProps<{
  label: string;
  /** Shown while there is no option at all; the select is disabled then. */
  emptyLabel: string;
  /** Shown while no option is in effect. */
  noneLabel: string;
  options: SelectOption[];
  modelValue: string | null;
}>();
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const id = useId();

/** What the closed select shows: the option in effect, or why there is none. */
const current = computed(() => {
  if (props.options.length === 0) {
    return props.emptyLabel;
  }
  const option = props.options.find((o) => o.value === props.modelValue);
  return option?.label ?? props.noneLabel;
});

const keys = createMenuKeys();

function onChange(event: Event): void {
  const element = event.target as HTMLSelectElement;
  const value = element.value;
  // Back to the placeholder: the next choice, the same entry included, changes the value again.
  element.value = '';
  if (value !== '' && !keys.browsing()) {
    emit('update:modelValue', value);
  }
}
</script>

<template>
  <div class="flex shrink-0 items-center">
    <label :for="id" class="sr-only">{{ label }}</label>
    <select
      :id="id"
      :title="label"
      :disabled="options.length === 0"
      value=""
      class="h-8 max-w-40 rounded-md bg-default px-2 text-sm text-default ring ring-inset ring-accented focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-not-allowed disabled:opacity-75"
      @pointerdown="keys.onPointerDown"
      @keydown="keys.onKeyDown"
      @keyup="keys.release"
      @blur="keys.release"
      @change="onChange"
    >
      <option value="" disabled selected>{{ current }}</option>
      <option v-for="option in options" :key="option.value" :value="option.value">
        {{ option.label }}
      </option>
    </select>
  </div>
</template>
