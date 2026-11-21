<script setup lang="ts">
// "Delete" of one entry of the Flows and the Recorder panel (RB-090-91): the trash button - its
// tooltip the command's title with its shortcut from the registry, disabled with the reason while
// the entry runs - and, while the panel's setting asks first (`automation.flows.confirmDelete`,
// `automation.recorder.confirmDelete`), the question inline in place of the button (design.md
// 4.4: a panel opens no dialog) with "Delete" in `error` and "Cancel" beside it; Cancel takes
// the focus, Esc answers it. Its own component because both panels confirm the same way.
import { nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { RECORDER_ICONS } from '../recorder/icons';
import { useShell } from '../state/shell';

const props = defineProps<{
  /** The entry's name. */
  name: string;
  /** The registry's command, for the tooltip's shortcut. */
  command: string;
  /** The question, naming what goes and what stays. */
  question: string;
  /** Ask before deleting. */
  confirm: boolean;
  /** Why it waits, or null. */
  blocked: string | null;
}>();
const emit = defineEmits<{ delete: [] }>();
const { t } = useI18n();
const shell = useShell();
const asking = ref(false);
const box = ref<HTMLElement | null>(null);

function hint(): string {
  const shortcut = shell.bindingOf(props.command);
  const label = t('panels.delete.label', { name: props.name });
  return shortcut === null ? label : t('panels.delete.shortcut', { label, shortcut });
}

function ask(): void {
  if (!props.confirm) {
    emit('delete');
    return;
  }
  asking.value = true;
  void nextTick(() => box.value?.querySelector<HTMLElement>('[data-testid="delete-no"]')?.focus());
}

// The entry started running while asked: the question goes, the button says why it waits.
watch(
  () => props.blocked,
  (now) => {
    if (now !== null) {
      asking.value = false;
    }
  },
);

function answer(yes: boolean): void {
  asking.value = false;
  if (yes) {
    emit('delete');
  }
}
</script>

<template>
  <UButton
    v-if="!asking"
    :icon="RECORDER_ICONS.entryDelete"
    size="sm"
    color="neutral"
    variant="ghost"
    :aria-label="t('panels.delete.label', { name })"
    :title="blocked ?? hint()"
    :disabled="blocked !== null"
    data-testid="delete"
    @click="ask"
  />
  <div
    v-else
    ref="box"
    role="alertdialog"
    :aria-label="t('panels.delete.label', { name })"
    class="flex basis-full flex-wrap items-center gap-2 py-1"
    data-testid="delete-confirm"
    @keydown.esc.stop="answer(false)"
  >
    <span class="text-sm">{{ question }}</span>
    <span class="ms-auto flex gap-1">
      <UButton
        size="sm"
        color="error"
        variant="solid"
        :label="t('panels.delete.confirm')"
        data-testid="delete-yes"
        @click="answer(true)"
      />
      <UButton
        size="sm"
        color="neutral"
        variant="outline"
        :label="t('panels.delete.cancel')"
        data-testid="delete-no"
        @click="answer(false)"
      />
    </span>
  </div>
</template>
