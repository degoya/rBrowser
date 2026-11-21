<script setup lang="ts">
// The steps of the recording that runs, live in the Recorder panel (RB-090-89,
// `automation.recorder.showLiveSteps`): each step as soon as the core has it (`recordingSteps`,
// `live.ts`), read-only - kind, element, name, value, the pause recorded before it and the
// pointer path's points, as the saved recording shows them (`StepRow.vue`). The newest step is
// marked and scrolled into sight; the count is announced politely. A recording continued from a
// step (RB-092-01) says after which step of the stored one its new steps land.
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import { livePauses, type LiveRecording } from './live';
import StepRow from './StepRow.vue';

const props = defineProps<{ live: LiveRecording }>();
const { t } = useI18n();
const list = ref<HTMLElement | null>(null);

const steps = computed(() => props.live.steps);
const pauses = computed(() => livePauses(steps.value));
const newest = computed(() => steps.value[steps.value.length - 1] ?? null);

/** The newest step in sight whenever it changes. */
watch(
  () => JSON.stringify(newest.value),
  () => {
    const id = newest.value?.id;
    if (id === undefined) {
      return;
    }
    void nextTick(() => {
      const row = list.value?.querySelector(`[data-testid="step-${id}"]`);
      if (row !== null && row !== undefined && typeof row.scrollIntoView === 'function') {
        row.scrollIntoView({ block: 'nearest' });
      }
    });
  },
  { immediate: true },
);
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col" data-testid="recorder-live">
    <div class="flex flex-wrap items-center gap-2 border-b border-default px-3 py-2">
      <UBadge color="error" variant="soft" size="sm" :label="t('recorder.panel.live.badge')" />
      <span class="truncate text-sm font-medium">{{ live.name }}</span>
      <span class="ms-auto text-xs text-muted" role="status" aria-live="polite" data-testid="recorder-live-count">{{
        t('recorder.panel.live.count', { count: steps.length })
      }}</span>
    </div>
    <p
      v-if="live.continueAfter !== undefined"
      class="mx-3 mt-2 rounded-md border border-dashed border-error bg-error/10 px-2 py-1 text-xs text-error"
      data-testid="recorder-live-continue"
    >
      {{ t('recorder.editor.continue_live') }}
    </p>
    <p v-if="steps.length === 0" class="p-3 text-sm text-muted" data-testid="recorder-live-empty">
      {{ t('recorder.panel.live.empty') }}
    </p>
    <ol ref="list" class="min-h-0 flex-1 overflow-auto" data-testid="recorder-live-steps">
      <StepRow
        v-for="(step, index) in steps"
        :key="step.id"
        :step="step"
        :index="index"
        :count="steps.length"
        :pause-ms="pauses.get(step.id) ?? 0"
        :newest="index === steps.length - 1"
        readonly
      />
    </ol>
  </div>
</template>
