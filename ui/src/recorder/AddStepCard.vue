<script setup lang="ts">
// The add card of the recording editor (RB-092-01) at the place a `+` chose: the step's fields
// (`StepEditor.vue`) and Cancel, Try (the new step alone in the active view, from the draft) and
// Add, which wait until the step has what its kind needs.
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { PickedElement } from '../ipc/protocol';
import { RECORDER_ICONS } from './icons';
import type { Step } from './recording';
import StepEditor from './StepEditor.vue';

defineProps<{
  seed: Step;
  candidate: Step | null;
  breakpoints: string[];
  urls: string[];
  picked: PickedElement | null;
  picking: boolean;
}>();
const emit = defineEmits<{ change: [step: Step | null]; pick: []; cancel: []; try: []; add: [] }>();
const { t } = useI18n();
</script>

<template>
  <li class="border-b border-default bg-elevated px-3 py-3" :aria-label="t('recorder.editor.add_title')" data-testid="add-step-card">
    <StepEditor
      :step="seed"
      :breakpoints="breakpoints"
      :urls="urls"
      :picked="picked"
      :picking="picking"
      id-prefix="step-add"
      @change="(next) => emit('change', next)"
      @pick="emit('pick')"
    >
      <template #actions>
        <div class="flex justify-end gap-2">
          <UButton size="sm" color="neutral" variant="ghost" :label="t('recorder.panel.cancel')" data-testid="step-add-cancel" @click="emit('cancel')" />
          <UButton
            size="sm"
            color="neutral"
            variant="outline"
            :icon="RECORDER_ICONS.stepTry"
            :label="t('recorder.editor.try_new')"
            :disabled="candidate === null"
            data-testid="step-add-try"
            @click="emit('try')"
          />
          <UButton
            size="sm"
            :icon="RECORDER_ICONS.stepAdd"
            :label="t('recorder.editor.add')"
            :disabled="candidate === null"
            data-testid="step-add-confirm"
            @click="emit('add')"
          />
        </div>
      </template>
    </StepEditor>
  </li>
</template>
