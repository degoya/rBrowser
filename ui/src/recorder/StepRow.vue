<script setup lang="ts">
// One step of the Recorder panel's step list (RB-090-60): its number, kind, element (its best
// selector), name and value - both editable in place -, the breakpoints it runs in (`onlyIn`)
// and the buttons that move it up or down or delete it. Every control is a native,
// Tab-reachable element with a name; no direct shortcut moves a step (`Alt+Up/Down` and
// `Ctrl+Shift+Up/Down` are reserved), the buttons do. A secret fill's value is never shown.
// While a recording plays (RB-090-84) the number of the step a view runs is marked, a failed
// step's number is red with the reason below it, and the recorded pause before the step shows
// beside its kind (`pauses.ts`).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import { RECORDER_ICONS } from './icons';
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
  }>(),
  { running: false, failure: null, pauseMs: 0 },
);
const emit = defineEmits<{
  rename: [name: string];
  value: [value: string];
  move: [by: -1 | 1];
  remove: [];
}>();
const { t } = useI18n();

const number = computed(() => props.index + 1);
const kind = computed(() => t(`recorder.panel.kind.${stepKind(props.step.action)}`));
const target = computed(() => targetText(props.step.action));
const value = computed(() => stepValue(props.step.action));
const secret = computed(() => props.step.action.type === 'fill' && props.step.action.secret === true);
const onlyIn = computed(() => props.step.onlyIn ?? []);
const marked = computed(() => props.running || props.failure !== null);
const failureText = computed(() =>
  props.failure === null || props.failure === ''
    ? t('recorder.panel.step.failed_plain')
    : t('recorder.panel.step.failed', { reason: props.failure }),
);
</script>

<template>
  <li
    class="flex flex-col gap-1.5 border-b border-default px-3 py-2"
    :class="{ 'bg-elevated': running && failure === null, 'bg-error/10': failure !== null }"
    :aria-current="running ? 'step' : undefined"
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
        :title="failure !== null ? failureText : t('recorder.panel.step.running')"
        data-testid="step-number"
      />
      <span v-else class="w-6 shrink-0 text-right text-xs text-muted tabular-nums" data-testid="step-number">{{ number }}</span>
      <UBadge color="neutral" variant="subtle" size="sm" :label="kind" data-testid="step-kind" />
      <span
        v-if="pauseMs > 0"
        class="shrink-0 text-xs text-muted tabular-nums"
        :title="t('recorder.panel.step.pause_hint')"
        data-testid="step-pause"
        >{{ t('recorder.panel.step.pause', { seconds: seconds(pauseMs) }) }}</span
      >
      <code v-if="target !== ''" class="min-w-0 truncate text-xs text-toned" :title="target" data-testid="step-target">
        {{ target }}
      </code>
      <span class="ms-auto flex shrink-0 items-center gap-0.5">
        <UButton
          :icon="RECORDER_ICONS.stepUp"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :disabled="index === 0"
          :aria-label="t('recorder.panel.step.up', { n: number })"
          :title="t('recorder.panel.step.up', { n: number })"
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
          :aria-label="t('recorder.panel.step.down', { n: number })"
          :title="t('recorder.panel.step.down', { n: number })"
          data-testid="step-down"
          @click="emit('move', 1)"
        />
        <UButton
          :icon="RECORDER_ICONS.stepRemove"
          color="error"
          variant="ghost"
          size="xs"
          square
          :aria-label="t('recorder.panel.step.remove', { n: number })"
          :title="t('recorder.panel.step.remove', { n: number })"
          data-testid="step-remove"
          @click="emit('remove')"
        />
      </span>
    </div>
    <div class="flex flex-wrap items-center gap-2 ps-8">
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
