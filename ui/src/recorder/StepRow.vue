<script setup lang="ts">
// One step of the Recorder panel's step list (RB-090-60): its number, kind, element (its best
// selector), name and value - both editable in place -, the breakpoints it runs in (`onlyIn`)
// and the buttons that move it up or down or delete it. Every control is a native,
// Tab-reachable element with a name; no direct shortcut moves a step (`Alt+Up/Down` and
// `Ctrl+Shift+Up/Down` are reserved), the buttons do. A secret fill's value is never shown.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import { RECORDER_ICONS } from './icons';
import { stepKind, stepValue, targetText, type Step } from './recording';

const props = defineProps<{ step: Step; index: number; count: number }>();
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
</script>

<template>
  <li class="flex flex-col gap-1.5 border-b border-default px-3 py-2" :data-testid="`step-${step.id}`">
    <div class="flex flex-wrap items-center gap-2">
      <span class="w-6 shrink-0 text-right text-xs text-muted tabular-nums">{{ number }}</span>
      <UBadge color="neutral" variant="subtle" size="sm" :label="kind" data-testid="step-kind" />
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
    <div v-if="onlyIn.length > 0" class="flex flex-wrap items-center gap-1 ps-8" data-testid="step-only-in">
      <span class="text-xs text-muted">{{ t('recorder.panel.step.only_in') }}</span>
      <UBadge v-for="breakpoint in onlyIn" :key="breakpoint" color="info" variant="soft" size="sm" :label="breakpoint" />
    </div>
  </li>
</template>
