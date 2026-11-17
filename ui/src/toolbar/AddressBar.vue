<script setup lang="ts">
// The address bar: Enter opens the URL in every view (navigation sync), Escape restores the
// current address. Refusals go to the shell's status line. It takes what the toolbar's row has
// left and never less than 240 px (`min-w-60`, `ADDRESS_MIN` of `fit.ts`): without that floor it
// was the row's only item that shrank, down to nothing (RB-080-52).
import { nextTick, ref, useId, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UInput from '@nuxt/ui/components/Input.vue';
import { ICONS } from '../icons';
import { normalizeUrlInput } from './url';

const props = defineProps<{ url: string; focusRequest?: number }>();
const emit = defineEmits<{ navigate: [url: string]; invalid: [key: string] }>();
const { t } = useI18n();

const id = useId();
const text = ref(props.url);
const editing = ref(false);
const invalid = ref(false);
const input = ref<{ inputRef?: HTMLInputElement | null } | null>(null);

watch(
  () => props.url,
  (url) => {
    if (!editing.value) {
      text.value = url;
    }
  },
);

function element(): HTMLInputElement | null {
  return input.value?.inputRef ?? null;
}

function focus(): void {
  const el = element();
  el?.focus();
  el?.select();
}

watch(
  () => props.focusRequest,
  () => nextTick(focus),
);

function submit(): void {
  const result = normalizeUrlInput(text.value);
  if (result.ok) {
    invalid.value = false;
    editing.value = false;
    text.value = result.url;
    emit('navigate', result.url);
  } else if (result.reason !== 'empty') {
    invalid.value = true;
    emit('invalid', INVALID_KEYS[result.reason]);
  }
}

/** The status line for an address that cannot be loaded. */
const INVALID_KEYS = {
  invalid: 'shell.toolbar.address_invalid',
  unsupported: 'shell.toolbar.address_unsupported',
} as const;

function revert(): void {
  text.value = props.url;
  editing.value = false;
  invalid.value = false;
}

defineExpose({ focus });
</script>

<template>
  <form class="min-w-60 flex-1" @submit.prevent="submit">
    <label :for="id" class="sr-only">{{ t('shell.toolbar.address') }}</label>
    <UInput
      :id="id"
      ref="input"
      v-model="text"
      type="text"
      size="sm"
      class="w-full"
      autocomplete="off"
      spellcheck="false"
      enterkeyhint="go"
      :icon="ICONS.url"
      :color="invalid ? 'error' : 'primary'"
      :highlight="invalid"
      :aria-invalid="invalid"
      :placeholder="t('shell.toolbar.address_placeholder')"
      @input="editing = true"
      @focus="element()?.select()"
      @keydown.esc.prevent="revert"
    />
  </form>
</template>
