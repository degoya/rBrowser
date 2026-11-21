<script setup lang="ts">
// The error toast at the top (RB-094-20, `toastModel.ts`): an error of an action the person
// started shows in the toolbar's strip as well - right-aligned, leaving the main menu `≡` free,
// one line, never over the views (the shell's page is the strip) - with the level as an icon
// and a word, the sentence (whole in the tooltip) and the message's buttons: Retry, Settings,
// Download. It goes after `SHOWN_MS`, unless the pointer or the focus is on it, with its close
// button or Escape; the status bar keeps the message. A Nuxt UI `UAlert` in a box of the strip's height: the Toaster
// floats its toasts in a viewport of their own, which a 44 px strip would cut off.
import { computed, onScopeDispose, shallowRef } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import { useShell } from '../state/shell';
import { levelOf } from '../statusbar/level';
import { STATUS_ICONS } from '../statusbar/icons';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { errorToastOn, latest, SHOWN_MS, toastOf, type Started, type ToastButton, type ToastModel } from './toastModel';

const props = withDefaults(defineProps<{ subscribe?: Subscribe; now?: () => number }>(), {
  subscribe: undefined,
  now: () => Date.now(),
});
const shell = useShell();
const { t } = useI18n();

// Shallow: a button's message holds JSON, too deep for Vue's type unwrapping (TS2589).
const toast = shallowRef<ToastModel | null>(null);
/** The buttons as the template draws them; a click finds the message by its kind. */
const buttons = computed(() => (toast.value?.buttons ?? []).map((button) => ({ kind: button.kind, labelKey: button.labelKey })));
/** The last chord of the leader key: a command the person started from the keyboard. */
let chord: Started | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
let held = false;

const look = levelOf('error');
const text = computed(() => (toast.value === null ? '' : t(toast.value.messageKey, toast.value.args)));

function stopTimer(): void {
  if (timer !== undefined) {
    clearTimeout(timer);
    timer = undefined;
  }
}

function startTimer(): void {
  stopTimer();
  if (!held) {
    timer = setTimeout(close, SHOWN_MS);
  }
}

function close(): void {
  stopTimer();
  held = false;
  toast.value = null;
}

function hold(on: boolean): void {
  held = on;
  if (on) {
    stopTimer();
  } else if (toast.value !== null) {
    startTimer();
  }
}

function press(kind: ToastButton['kind']): void {
  const button = toast.value?.buttons.find((candidate) => candidate.kind === kind);
  close();
  if (button !== undefined) {
    void shell.send(button.message);
  }
}

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'whichKey') {
    chord = { at: props.now() };
  } else if (message.type === 'notification' && errorToastOn(shell.settings.value)) {
    const next = toastOf(message, latest(shell.lastRun.value, chord), props.now());
    if (next !== null) {
      toast.value = next;
      startTimer();
    }
  }
});
onScopeDispose(() => {
  stop();
  stopTimer();
});
</script>

<template>
  <div
    v-if="toast !== null"
    class="fixed right-12 top-1 z-40 h-9 w-[min(40rem,calc(100vw-4rem))]"
    data-testid="error-toast"
    @pointerenter="hold(true)"
    @pointerleave="hold(false)"
    @focusin="hold(true)"
    @focusout="hold(false)"
    @keydown.escape.prevent="close"
  >
    <UAlert
      role="alert"
      color="error"
      variant="outline"
      orientation="horizontal"
      :icon="look.icon"
      :title="text"
      :ui="{ root: 'h-9 items-center gap-2 bg-default px-2 py-0 shadow-md', title: 'truncate text-sm font-normal text-default', icon: 'size-4', actions: 'gap-1' }"
      data-testid="error-toast-alert"
    >
      <template #title>
        <span class="flex min-w-0 items-center gap-1.5" :title="text">
          <span class="shrink-0 font-medium text-error">{{ t(look.wordKey) }}</span>
          <span class="min-w-0 truncate" data-testid="error-toast-text">{{ text }}</span>
        </span>
      </template>
      <template #actions>
        <UButton
          v-for="button in buttons"
          :key="button.kind"
          size="xs"
          :color="button.kind === 'retry' ? 'primary' : 'neutral'"
          :variant="button.kind === 'retry' ? 'soft' : 'ghost'"
          :label="t(button.labelKey)"
          :data-testid="`error-toast-${button.kind}`"
          @click="press(button.kind)"
        />
        <UButton
          :icon="STATUS_ICONS.statusClose"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :aria-label="t('shell.toast.close')"
          :title="t('shell.toast.close')"
          data-testid="error-toast-close"
          @click="close"
        />
      </template>
    </UAlert>
  </div>
</template>
