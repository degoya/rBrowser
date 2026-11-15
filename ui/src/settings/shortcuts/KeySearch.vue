<script setup lang="ts">
// "Search by keys" in the shortcut editor (RB-060-11): press a combination and the editor shows
// the command on it - after the leader, every chord the typed keys start. Escape cancels; the
// result stays as a chip until it is cleared. When the capture ends or the chip is cleared, the
// focus goes back to "Search by keys" (RB-090-23).
import { nextTick, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UKbd from '@nuxt/ui/components/Kbd.vue';
import { SETTINGS_ICONS } from '../icons';
import { Recorder } from './record';

const pressed = defineModel<string | null>({ required: true });
const { t } = useI18n();
const capturing = ref(false);
const typed = ref('');
const capture = ref<HTMLElement | null>(null);
const searchButton = ref<{ $el: HTMLElement } | null>(null);
const recorder = new Recorder();

async function start(): Promise<void> {
  recorder.reset();
  typed.value = '';
  capturing.value = true;
  await nextTick();
  capture.value?.focus();
}

/** The focus back to "Search by keys", once it is drawn again. */
async function focusSearch(): Promise<void> {
  await nextTick();
  searchButton.value?.$el.focus();
}

function clear(): void {
  pressed.value = null;
  void focusSearch();
}

function onKey(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
  const step = recorder.feed(event);
  typed.value = recorder.text();
  if (step === 'cancel') {
    capturing.value = false;
    void focusSearch();
  } else if (step === 'done' || (step === 'more' && recorder.text().includes(' '))) {
    // A leader prefix already narrows the list; the next key narrows it further.
    pressed.value = recorder.text();
    if (step === 'done') {
      capturing.value = false;
      void focusSearch();
    }
  }
}
</script>

<template>
  <div class="flex items-center gap-2">
    <div
      v-if="capturing"
      ref="capture"
      tabindex="0"
      role="textbox"
      aria-live="polite"
      :aria-label="t('settings.shortcuts.key_search_prompt')"
      class="min-w-48 rounded-md bg-elevated px-2 py-1 font-mono text-sm ring-2 ring-primary focus-visible:outline-none"
      data-testid="key-search-capture"
      @keydown="onKey"
      @blur="capturing = false"
    >
      {{ typed || t('settings.shortcuts.key_search_prompt') }}
    </div>
    <UButton
      v-else
      ref="searchButton"
      :icon="SETTINGS_ICONS.record"
      color="neutral"
      variant="soft"
      size="sm"
      :label="t('settings.shortcuts.key_search')"
      data-testid="key-search"
      @click="start"
    />
    <span v-if="pressed !== null && !capturing" class="flex items-center gap-1 text-sm" data-testid="key-search-active">
      {{ t('settings.shortcuts.key_search_label') }}
      <UKbd :value="pressed" size="md" />
      <UButton
        :icon="SETTINGS_ICONS.close"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        :aria-label="t('settings.shortcuts.key_search_clear')"
        :title="t('settings.shortcuts.key_search_clear')"
        data-testid="key-search-clear"
        @click="clear"
      />
    </span>
  </div>
</template>
