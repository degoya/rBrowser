<script setup lang="ts">
// The address field of a view whose navigation is decoupled (RB-020-06,
// `breakpoints[].syncNavigation: false` or `Leader S U`): it takes the place of the label in the
// cell's label strip, in the grid gap above the view, and loads its address in that view only.
// Enter loads, Escape restores the view's address; a refused address marks the field.
import { ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { normalizeUrlInput } from '../toolbar/url';

const props = defineProps<{ url: string; label: string }>();
const emit = defineEmits<{ navigate: [url: string] }>();
const { t } = useI18n();

const id = useId();
const text = ref(props.url);
const editing = ref(false);
const invalid = ref(false);

watch(
  () => props.url,
  (url) => {
    if (!editing.value) {
      text.value = url;
    }
  },
);

function submit(): void {
  const result = normalizeUrlInput(text.value);
  if (!result.ok) {
    invalid.value = true;
    return;
  }
  invalid.value = false;
  editing.value = false;
  text.value = result.url;
  emit('navigate', result.url);
}

function revert(): void {
  text.value = props.url;
  editing.value = false;
  invalid.value = false;
}
</script>

<template>
  <form class="flex min-w-0 flex-1" data-testid="view-address" @submit.prevent="submit">
    <label :for="id" class="sr-only">{{ t('sync.address.label', { label }) }}</label>
    <input
      :id="id"
      v-model="text"
      type="text"
      class="h-4 min-w-0 flex-1 rounded-sm border border-default bg-default px-1 text-xs text-default focus-visible:outline-2 focus-visible:outline-primary"
      :class="invalid ? 'border-error' : ''"
      autocomplete="off"
      spellcheck="false"
      enterkeyhint="go"
      :aria-invalid="invalid"
      :title="invalid ? t('sync.address.invalid') : undefined"
      :placeholder="t('sync.address.placeholder')"
      data-testid="view-address-input"
      @input="editing = true"
      @focus="($event.target as HTMLInputElement).select()"
      @keydown.esc.prevent="revert"
    />
  </form>
</template>
