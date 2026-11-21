<script setup lang="ts">
// A step's buttons in the Recorder panel: up, down and delete (RB-090-60), and the editor's
// (RB-092-01) - "Try" in the active view, "Edit" (pressed while the step is open) and "More": play
// from here, continue recording from here (once the recording is saved), duplicate, skip or
// include. Each is an icon with its name as label and tooltip, the registry's shortcut beside it
// where the command has one (design.md 3, 4.5).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UDropdownMenu, { type DropdownMenuItem } from '@nuxt/ui/components/DropdownMenu.vue';
import { chordKeys } from '../panels/shortcut';
import { useShell } from '../state/shell';
import { RECORDER_ICONS } from './icons';

const props = withDefaults(
  defineProps<{
    index: number;
    count: number;
    editing?: boolean;
    disabled?: boolean;
    /** Why "Continue recording from here" waits (unsaved edits, a recording runs); null: it can. */
    continueBlocked?: string | null;
  }>(),
  { editing: false, disabled: false, continueBlocked: null },
);
const emit = defineEmits<{
  move: [by: -1 | 1];
  remove: [];
  try: [];
  edit: [];
  playFrom: [];
  continueFrom: [];
  duplicate: [];
  toggle: [];
}>();
const { t } = useI18n();
const shell = useShell();
const n = computed(() => props.index + 1);

/** `text` with the command's shortcut from the registry, when it has one. */
function hinted(text: string, command: string): string {
  const shortcut = shell.bindingOf(command);
  return shortcut === null ? text : t('recorder.editor.with_shortcut', { text, shortcut });
}

const tryLabel = computed(() => hinted(t('recorder.editor.try', { n: n.value }), 'recorder.step_try'));
const editLabel = computed(() => t(props.editing ? 'recorder.editor.close' : 'recorder.editor.edit', { n: n.value }));
const items = computed<DropdownMenuItem[][]>(() => [
  [
    {
      label: t('recorder.editor.play_from'),
      icon: RECORDER_ICONS.stepPlayFrom,
      kbds: chordKeys(shell.bindingOf('recorder.play_from')),
      onSelect: () => emit('playFrom'),
    },
    {
      label: props.continueBlocked ?? t('recorder.editor.continue_from'),
      icon: RECORDER_ICONS.stepContinue,
      disabled: props.continueBlocked !== null,
      kbds: chordKeys(shell.bindingOf('recorder.continue_from')),
      onSelect: () => emit('continueFrom'),
    },
  ],
  [
    { label: t('recorder.editor.duplicate'), icon: RECORDER_ICONS.stepDuplicate, onSelect: () => emit('duplicate') },
    {
      label: props.disabled ? t('recorder.editor.include') : t('recorder.editor.skip'),
      icon: RECORDER_ICONS.stepSkip,
      kbds: chordKeys(shell.bindingOf('recorder.step_toggle')),
      onSelect: () => emit('toggle'),
    },
  ],
]);
</script>

<template>
  <span class="ms-auto flex shrink-0 items-center gap-0.5" data-testid="step-actions">
    <UButton
      :icon="RECORDER_ICONS.stepTry"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      :aria-label="tryLabel"
      :title="tryLabel"
      data-testid="step-try"
      @click="emit('try')"
    />
    <UButton
      :icon="RECORDER_ICONS.stepEdit"
      :color="editing ? 'primary' : 'neutral'"
      :variant="editing ? 'soft' : 'ghost'"
      size="xs"
      square
      :aria-pressed="editing ? 'true' : 'false'"
      :aria-label="editLabel"
      :title="editLabel"
      data-testid="step-edit"
      @click="emit('edit')"
    />
    <UButton
      :icon="RECORDER_ICONS.stepUp"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      :disabled="index === 0"
      :aria-label="t('recorder.panel.step.up', { n })"
      :title="t('recorder.panel.step.up', { n })"
      data-testid="step-up"
      @click="emit('move', -1)"
    />
    <UButton
      :icon="RECORDER_ICONS.stepDown"
      color="neutral"
      variant="ghost"
      size="xs"
      square
      :disabled="index === count - 1"
      :aria-label="t('recorder.panel.step.down', { n })"
      :title="t('recorder.panel.step.down', { n })"
      data-testid="step-down"
      @click="emit('move', 1)"
    />
    <UDropdownMenu :items="items" :content="{ align: 'end', collisionPadding: 4 }" :modal="false">
      <UButton
        :icon="RECORDER_ICONS.stepMore"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        :aria-label="t('recorder.editor.more', { n })"
        :title="t('recorder.editor.more', { n })"
        data-testid="step-more"
      />
    </UDropdownMenu>
    <UButton
      :icon="RECORDER_ICONS.stepRemove"
      color="error"
      variant="ghost"
      size="xs"
      square
      :aria-label="t('recorder.panel.step.remove', { n })"
      :title="t('recorder.panel.step.remove', { n })"
      data-testid="step-remove"
      @click="emit('remove')"
    />
  </span>
</template>
