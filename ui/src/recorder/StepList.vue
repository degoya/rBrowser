<script setup lang="ts">
// The Recorder panel's step list with the recording editor (RB-092-01): each step (`StepRow.vue`),
// a `+` between two steps and "Add step" at the end, the add card at the place chosen and one step
// open in the editor at a time (`StepEditor.vue`). Every change is an edit of the panel's draft
// (`edit`), saved with the rest. "Pick in view" asks the core (`pickElement`) and hands the element
// to the editor that asked. A shortcut's editor command (`runCommand`, `recorderStepCommand`)
// acts on the step in focus - the one last focused, else the last.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { PickedElement } from '../ipc/protocol';
import { useShell } from '../state/shell';
import { RECORDER_ICONS } from './icons';
import { deleteStep, moveStep, renameStep, setStepValue, type Recording, type Step } from './recording';
import { duplicateStep, insertStep, newStep, replaceStep, setDisabled, type After } from './recordingEdit';
import AddStepCard from './AddStepCard.vue';
import StepEditor from './StepEditor.vue';
import StepRow from './StepRow.vue';

const props = withDefaults(
  defineProps<{
    recording: Recording;
    runningIds: Set<number>;
    failed: Record<number, string>;
    /** By step id: the recorded pause before it, shown while nothing is unsaved. */
    pauses: Map<number, number>;
    dirty: boolean;
    /** A recording runs (its name): "Continue recording from here" waits. */
    recordingNow?: string;
    /** `automation.recorder.insertedStepPauseMs`: an added step's pause. */
    insertedPause?: number;
    /** URLs to propose for a URL. */
    urls?: string[];
    pickElement: () => Promise<PickedElement | null>;
  }>(),
  { recordingNow: undefined, insertedPause: 500, urls: () => [] },
);
const emit = defineEmits<{
  edit: [change: (recording: Recording) => Recording];
  try: [stepId: number, draft: Recording | null];
  playFrom: [stepId: number];
  continueFrom: [stepId: number];
}>();
const { t } = useI18n();

const steps = computed(() => props.recording.steps);
const breakpoints = computed(() => props.recording.breakpoints.map((breakpoint) => breakpoint.id));
const editingId = ref<number | null>(null);
/** Where the add card is; `undefined` while none is open. */
const adding = ref<After | undefined>(undefined);
const seed = ref<Step | null>(null);
const candidate = ref<Step | null>(null);
const focusedId = ref<number | null>(null);
const picking = ref(false);
const picked = ref<PickedElement | null>(null);
const shell = useShell();
/** "Add step"'s tooltip: what it does and its shortcut from the registry. */
const addHint = computed(() => {
  const shortcut = shell.bindingOf('recorder.step_add');
  const text = t('recorder.editor.add_hint');
  return shortcut === null ? text : t('recorder.editor.with_shortcut', { text, shortcut });
});
const continueBlocked = computed(() => {
  if (props.recordingNow !== undefined) {
    return t('recorder.editor.continue_recording');
  }
  return props.dirty ? t('recorder.editor.continue_unsaved') : null;
});

function focused(): number | null {
  const known = steps.value.some((step) => step.id === focusedId.value);
  return known ? focusedId.value : (steps.value.at(-1)?.id ?? null);
}

function openAdd(after: After): void {
  editingId.value = null;
  picked.value = null;
  adding.value = after;
  seed.value = newStep(props.recording, after, { type: 'click', target: { selectors: [] } }, props.insertedPause);
  candidate.value = null;
}

function closeAdd(): void {
  adding.value = undefined;
  seed.value = null;
  candidate.value = null;
}

function add(): void {
  const step = candidate.value;
  const after = adding.value;
  if (step === null || after === undefined) {
    return;
  }
  emit('edit', (recording) => insertStep(recording, after, step));
  focusedId.value = step.id;
  closeAdd();
}

function tryCandidate(): void {
  const step = candidate.value;
  if (step !== null && adding.value !== undefined) {
    emit('try', step.id, insertStep(props.recording, adding.value, step));
  }
}

function toggleEdit(id: number): void {
  closeAdd();
  picked.value = null;
  editingId.value = editingId.value === id ? null : id;
}

function changed(id: number, step: Step | null): void {
  if (step !== null) {
    emit('edit', (recording) => replaceStep(recording, id, step));
  }
}

function toggle(id: number): void {
  const step = steps.value.find((known) => known.id === id);
  if (step !== undefined) {
    emit('edit', (recording) => setDisabled(recording, id, step.disabled !== true));
  }
}

function tryStep(id: number): void {
  emit('try', id, props.dirty ? props.recording : null);
}

async function pick(): Promise<void> {
  if (picking.value) {
    return;
  }
  picking.value = true;
  try {
    picked.value = await props.pickElement();
  } finally {
    picking.value = false;
  }
}

/** A shortcut's editor command on the step in focus (`recorderStepCommand`). */
function runCommand(command: string): void {
  const id = focused();
  if (command === 'recorder.step_add') {
    openAdd(id);
    return;
  }
  if (command === 'recorder.pick_element') {
    if (adding.value !== undefined || editingId.value !== null) {
      void pick();
    }
    return;
  }
  if (id === null) {
    return;
  }
  if (command === 'recorder.step_try') {
    tryStep(id);
  } else if (command === 'recorder.play_from') {
    emit('playFrom', id);
  } else if (command === 'recorder.continue_from' && continueBlocked.value === null) {
    emit('continueFrom', id);
  } else if (command === 'recorder.step_toggle') {
    toggle(id);
  }
}

defineExpose({ runCommand });
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <p v-if="steps.length === 0" class="p-3 text-sm text-muted">{{ t('recorder.panel.no_steps') }}</p>
    <ol class="min-h-0 flex-1 overflow-auto" data-testid="recorder-steps">
      <template v-for="(step, index) in steps" :key="step.id">
        <li v-if="index === 0" class="relative h-0" data-testid="insert-gap-start">
          <UButton
            :icon="RECORDER_ICONS.stepAdd"
            size="xs"
            color="neutral"
            variant="outline"
            square
            class="absolute start-6 -top-1 z-10 rounded-full"
            :aria-label="t('recorder.editor.add_first')"
            :title="t('recorder.editor.add_first')"
            @click="openAdd(0)"
          />
        </li>
        <AddStepCard
          v-if="adding === 0 && index === 0 && seed !== null"
          :seed="seed"
          :candidate="candidate"
          :breakpoints="breakpoints"
          :urls="urls"
          :picked="picked"
          :picking="picking"
          @change="(next) => (candidate = next)"
          @pick="pick"
          @cancel="closeAdd"
          @try="tryCandidate"
          @add="add"
        />
        <StepRow
          :step="step"
          :index="index"
          :count="steps.length"
          :running="runningIds.has(step.id)"
          :failure="failed[step.id] ?? null"
          :pause-ms="dirty ? 0 : (pauses.get(step.id) ?? 0)"
          :editing="editingId === step.id"
          :continue-blocked="continueBlocked"
          @focusin="focusedId = step.id"
          @rename="(name) => emit('edit', (recording) => renameStep(recording, step.id, name))"
          @value="(value) => emit('edit', (recording) => setStepValue(recording, step.id, value))"
          @move="(by) => emit('edit', (recording) => moveStep(recording, step.id, by))"
          @remove="emit('edit', (recording) => deleteStep(recording, step.id))"
          @try="tryStep(step.id)"
          @edit="toggleEdit(step.id)"
          @play-from="emit('playFrom', step.id)"
          @continue-from="emit('continueFrom', step.id)"
          @duplicate="emit('edit', (recording) => duplicateStep(recording, step.id))"
          @toggle="toggle(step.id)"
        >
          <template #editor>
            <StepEditor
              :step="step"
              :breakpoints="breakpoints"
              :urls="urls"
              :picked="picked"
              :picking="picking"
              :id-prefix="`step-${step.id}`"
              @change="(next) => changed(step.id, next)"
              @pick="pick"
            >
              <template #actions>
                <div class="flex justify-end">
                  <UButton size="sm" color="neutral" variant="outline" :label="t('recorder.editor.done')" data-testid="step-editor-done" @click="editingId = null" />
                </div>
              </template>
            </StepEditor>
          </template>
        </StepRow>
        <li class="relative h-0" :data-testid="`insert-gap-${step.id}`">
          <UButton
            :icon="RECORDER_ICONS.stepAdd"
            size="xs"
            :color="adding === step.id ? 'primary' : 'neutral'"
            :variant="adding === step.id ? 'solid' : 'outline'"
            square
            class="absolute start-6 -top-3 z-10 rounded-full"
            :aria-expanded="adding === step.id ? 'true' : 'false'"
            :aria-label="t('recorder.editor.add_after', { n: index + 1 })"
            :title="t('recorder.editor.add_after', { n: index + 1 })"
            :data-testid="`step-insert-${step.id}`"
            @click="openAdd(step.id)"
          />
        </li>
        <AddStepCard
          v-if="adding === step.id && seed !== null"
          :seed="seed"
          :candidate="candidate"
          :breakpoints="breakpoints"
          :urls="urls"
          :picked="picked"
          :picking="picking"
          @change="(next) => (candidate = next)"
          @pick="pick"
          @cancel="closeAdd"
          @try="tryCandidate"
          @add="add"
        />
      </template>
      <AddStepCard
        v-if="adding === null && seed !== null"
        :seed="seed"
        :candidate="candidate"
        :breakpoints="breakpoints"
        :urls="urls"
        :picked="picked"
        :picking="picking"
        @change="(next) => (candidate = next)"
        @pick="pick"
        @cancel="closeAdd"
        @try="tryCandidate"
        @add="add"
      />
    </ol>
    <div class="flex items-center gap-2 border-t border-default px-3 py-2">
      <UButton
        :icon="RECORDER_ICONS.stepAdd"
        size="sm"
        color="neutral"
        variant="outline"
        :label="t('recorder.editor.add_end')"
        :title="addHint"
        :aria-expanded="adding === null ? 'true' : 'false'"
        data-testid="step-add-end"
        @click="openAdd(null)"
      />
    </div>
  </div>
</template>
