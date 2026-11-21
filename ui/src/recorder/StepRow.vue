<script setup lang="ts">
// One step of the Recorder panel's step list (RB-090-60): its number, kind, element (its best
// selector), name and value - both editable in place -, the breakpoints it runs in (`onlyIn`)
// and the buttons that move it up or down or delete it. Every control is a native,
// Tab-reachable element with a name; no direct shortcut moves a step (`Alt+Up/Down` and
// `Ctrl+Shift+Up/Down` are reserved), the buttons do. A secret fill's value is never shown.
// While a recording plays (RB-090-84) the number of the step a view runs is marked, a failed
// step's number is red with the reason below it, and the recorded pause before the step shows
// beside its kind (`pauses.ts`). A step with the pointer's way names its points (RB-090-87).
// `readonly` (the live steps of a recording that runs, RB-090-89) shows name and value as text,
// without the buttons; `newest` marks the step recorded last. The editor (RB-092-01): the step's
// buttons are `StepActions.vue`; a step switched off shows struck through with "Skipped" (the word,
// not colour alone), and a step open in the editor shows its editor in place of name and value.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import { RECORDER_ICONS } from './icons';
import StepActions from './StepActions.vue';
import { seconds } from './pauses';
import { stepKind, stepValue, targetText, type Step } from './recording';

const props = withDefaults(
  defineProps<{
    step: Step;
    index: number;
    count: number;
    /** A view runs this step now. */
    running?: boolean;
    /** Why the step failed in the last run; null when it did not. */
    failure?: string | null;
    /** The pause before the step at the pace "As recorded", in ms. */
    pauseMs?: number;
    /** Shown, not edited: no buttons, name and value as text. */
    readonly?: boolean;
    /** The step recorded last. */
    newest?: boolean;
    /** Open in the editor (RB-092-01): its editor in the `editor` slot. */
    editing?: boolean;
    /** Why "Continue recording from here" waits; null: it can. */
    continueBlocked?: string | null;
  }>(),
  { running: false, failure: null, pauseMs: 0, readonly: false, newest: false, editing: false, continueBlocked: null },
);
const emit = defineEmits<{
  rename: [name: string];
  value: [value: string];
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

const number = computed(() => props.index + 1);
const kind = computed(() => t(`recorder.panel.kind.${stepKind(props.step.action)}`));
const target = computed(() => targetText(props.step.action));
const value = computed(() => stepValue(props.step.action));
const secret = computed(() => props.step.action.type === 'fill' && props.step.action.secret === true);
const onlyIn = computed(() => props.step.onlyIn ?? []);
const pathPoints = computed(() => props.step.path?.length ?? 0);
const off = computed(() => props.step.disabled === true);
const marked = computed(() => props.running || props.failure !== null || props.newest);
const failureText = computed(() =>
  props.failure === null || props.failure === ''
    ? t('recorder.panel.step.failed_plain')
    : t('recorder.panel.step.failed', { reason: props.failure }),
);
const numberTitle = computed(() => {
  if (props.failure !== null) {
    return failureText.value;
  }
  return props.running ? t('recorder.panel.step.running') : t('recorder.panel.step.newest');
});
</script>

<template>
  <li
    class="flex flex-col gap-1.5 border-b border-default px-3 py-2"
    :class="{
      'bg-elevated': (running || newest || editing) && failure === null,
      'bg-error/10': failure !== null,
      'border-s-3 border-s-primary': editing,
    }"
    :data-disabled="off ? 'true' : undefined"
    :aria-current="running ? 'step' : newest ? 'true' : undefined"
    :data-newest="newest ? 'true' : undefined"
    :data-testid="`step-${step.id}`"
  >
    <div class="flex flex-wrap items-center gap-2">
      <UBadge
        v-if="marked"
        :color="failure !== null ? 'error' : 'primary'"
        variant="solid"
        size="sm"
        class="min-w-6 shrink-0 justify-center tabular-nums"
        :label="String(number)"
        :title="numberTitle"
        data-testid="step-number"
      />
      <span v-else class="w-6 shrink-0 text-right text-xs text-muted tabular-nums" data-testid="step-number">{{ number }}</span>
      <UBadge color="neutral" variant="subtle" size="sm" :label="kind" :class="{ 'line-through opacity-60': off }" data-testid="step-kind" />
      <UBadge
        v-if="off"
        color="neutral"
        variant="outline"
        size="sm"
        :icon="RECORDER_ICONS.stepSkip"
        :label="t('recorder.editor.skipped')"
        :title="t('recorder.editor.skipped_hint')"
        class="border-dashed"
        data-testid="step-skipped"
      />
      <span
        v-if="pauseMs > 0"
        class="shrink-0 text-xs text-muted tabular-nums"
        :title="t('recorder.panel.step.pause_hint')"
        data-testid="step-pause"
        >{{ t('recorder.panel.step.pause', { seconds: seconds(pauseMs) }) }}</span
      >
      <span
        v-if="pathPoints > 0"
        class="shrink-0 text-xs text-muted tabular-nums"
        :title="t('recorder.panel.step.path_hint')"
        data-testid="step-path"
        >{{ t('recorder.panel.step.path', { count: pathPoints }) }}</span
      >
      <code v-if="target !== ''" class="min-w-0 truncate text-xs text-toned" :title="target" data-testid="step-target">
        {{ target }}
      </code>
      <StepActions
        v-if="!readonly"
        :index="index"
        :count="count"
        :editing="editing"
        :disabled="off"
        :continue-blocked="continueBlocked"
        @move="(by) => emit('move', by)"
        @remove="emit('remove')"
        @try="emit('try')"
        @edit="emit('edit')"
        @play-from="emit('playFrom')"
        @continue-from="emit('continueFrom')"
        @duplicate="emit('duplicate')"
        @toggle="emit('toggle')"
      />
    </div>
    <div v-if="editing && !readonly" class="ps-8" data-testid="step-editing"><slot name="editor" /></div>
    <div v-else-if="readonly" class="flex flex-wrap items-center gap-2 ps-8 text-sm">
      <span v-if="step.name !== undefined" class="font-medium" data-testid="step-name-text">{{ step.name }}</span>
      <span v-if="value !== null" class="min-w-0 truncate text-toned" data-testid="step-value-text">{{ value }}</span>
      <span v-else-if="secret" class="text-xs text-muted" data-testid="step-secret">
        {{ t('recorder.panel.step.secret') }}
      </span>
    </div>
    <div v-else class="flex flex-wrap items-center gap-2 ps-8">
      <UInput
        :model-value="step.name ?? ''"
        size="sm"
        class="min-w-40 flex-1"
        :placeholder="t('recorder.panel.step.name_placeholder')"
        :aria-label="t('recorder.panel.step.name', { n: number })"
        data-testid="step-name"
        @update:model-value="(text) => emit('rename', String(text))"
      />
      <UInput
        v-if="value !== null"
        :model-value="value"
        size="sm"
        class="min-w-40 flex-1"
        :aria-label="t('recorder.panel.step.value', { n: number })"
        data-testid="step-value"
        @update:model-value="(text) => emit('value', String(text))"
      />
      <span v-else-if="secret" class="text-xs text-muted" data-testid="step-secret">
        {{ t('recorder.panel.step.secret') }}
      </span>
    </div>
    <p v-if="failure !== null" class="ps-8 text-xs text-error" data-testid="step-failure">{{ failureText }}</p>
    <div v-if="onlyIn.length > 0" class="flex flex-wrap items-center gap-1 ps-8" data-testid="step-only-in">
      <span class="text-xs text-muted">{{ t('recorder.panel.step.only_in') }}</span>
      <UBadge v-for="breakpoint in onlyIn" :key="breakpoint" color="info" variant="soft" size="sm" :label="breakpoint" />
    </div>
  </li>
</template>
