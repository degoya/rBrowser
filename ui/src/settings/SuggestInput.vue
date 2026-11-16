<script setup lang="ts">
// A free text that proposes values (design.md 4, rule 7, "open, but usually known"; RB-080-88):
// a Nuxt UI `UInputMenu` in autocomplete mode, so the text stays free and a proposal fills it -
// the `x-options` of a string, the installations the core found for a path (RB-080-32). It
// commits like the native field it replaces: on Enter, on leaving it, or when a proposal is
// chosen, never per keystroke; an unchanged text commits nothing.
import { nextTick, ref, watch } from 'vue';
import UInputMenu from '@nuxt/ui/components/InputMenu.vue';

export interface Suggestion {
  value: string;
  label: string;
}

const props = defineProps<{
  value: string;
  items: Suggestion[];
  inputId: string;
  /** The accessible name. */
  label: string;
  describedBy?: string;
  disabled?: boolean;
  placeholder?: string;
  invalid?: boolean;
}>();
const emit = defineEmits<{ commit: [text: string] }>();

const draft = ref(props.value);
let committed = props.value;
watch(
  () => props.value,
  (value) => {
    draft.value = value;
    committed = value;
  },
);

function commit(): void {
  if (draft.value === committed) {
    return;
  }
  committed = draft.value;
  emit('commit', draft.value);
}

/** Enter commits once the menu took its choice, so a highlighted proposal is what is saved. */
function onEnter(): void {
  void nextTick(commit);
}
</script>

<template>
  <UInputMenu
    :id="inputId"
    v-model="draft"
    mode="autocomplete"
    :items="items"
    value-key="value"
    :filter-fields="['label', 'value']"
    :placeholder="placeholder"
    :disabled="disabled"
    :aria-label="label"
    :aria-describedby="describedBy"
    :aria-invalid="invalid ? 'true' : undefined"
    spellcheck="false"
    class="w-full"
    data-testid="suggestions"
    @blur="commit"
    @keydown.enter="onEnter"
  />
</template>
