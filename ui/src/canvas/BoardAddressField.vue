<script setup lang="ts">
// The address field of a decoupled view's label on the canvas board (RB-092-03): in the label,
// in the address's place, never over a view. The core opens it (`uiCommand` `view.edit_address`)
// with the view's address selected. Enter sends the typed text for this view (`addressEdit.ts`);
// the core checks it as the address bar's input, loads it in this view only and closes nothing
// itself - the field closes once the core took it, and stays marked while the status bar names
// the core's reason for a refusal. Escape and a focus elsewhere close it. Its press, release
// and keys never reach the label (no drag, no step).
import { nextTick, onMounted, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UInput from '@nuxt/ui/components/Input.vue';
import type { ViewId } from '../ipc/protocol';
import { useShell } from '../state/shell';
import { submitMessage } from './addressEdit';

const props = defineProps<{
  view: ViewId;
  /** The address it starts with. */
  url: string;
  /** The view's label, for the field's accessible name. */
  label: string;
}>();

const shell = useShell();
const { t } = useI18n();
const id = useId();
const text = ref(props.url);
const invalid = ref(false);
let sending = false;
const input = ref<{ inputRef?: HTMLInputElement | null } | null>(null);

onMounted(() =>
  nextTick(() => {
    const element = input.value?.inputRef ?? null;
    element?.focus();
    element?.select();
  }),
);

async function submit(): Promise<void> {
  if (sending) {
    return;
  }
  sending = true;
  // A refusal is no news from here: the core names its reason in the status bar.
  const taken = await shell.ask(submitMessage(props.view, text.value));
  sending = false;
  if (taken) {
    shell.closeAddressEdit();
  } else {
    invalid.value = true;
  }
}

function onKey(event: KeyboardEvent): void {
  event.stopPropagation();
  if (event.key === 'Escape') {
    event.preventDefault();
    shell.closeAddressEdit();
  }
}

function onBlur(): void {
  if (!sending) {
    shell.closeAddressEdit();
  }
}
</script>

<template>
  <form
    class="flex min-w-24 flex-1 items-center"
    :data-testid="`board-address-form-${view}`"
    @submit.prevent="submit"
    @pointerdown.stop
    @pointerup.stop
    @click.stop
  >
    <label :for="id" class="sr-only">{{ t('sync.address.label', { label }) }}</label>
    <UInput
      :id="id"
      ref="input"
      v-model="text"
      type="text"
      size="xs"
      class="w-full"
      autocomplete="off"
      spellcheck="false"
      enterkeyhint="go"
      :color="invalid ? 'error' : 'primary'"
      :highlight="invalid"
      :aria-invalid="invalid"
      :placeholder="t('cells.address.placeholder')"
      :data-testid="`board-address-field-${view}`"
      @input="invalid = false"
      @keydown="onKey"
      @blur="onBlur"
    />
  </form>
</template>
