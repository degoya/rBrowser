<script setup lang="ts">
// The toolbar's recording indicator (RB-050-01): shown only while a recording runs, in the
// toolbar strip (its own CEF view, never over a browser view). The first button pauses or
// resumes (`recorder.pause`), the second stops and saves (`recorder.stop`); both are native,
// Tab-reachable buttons with a label; the first one's visible label is its name, the title its
// description (label in name, RB-090-23).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import { useShell } from '../state/shell';
import { recorderIndicator } from './indicator';

const shell = useShell();
const { t } = useI18n();

const indicator = computed(() => recorderIndicator(shell.state.value));
const args = computed(() => ({
  name: indicator.value?.name ?? '',
  steps: indicator.value?.steps ?? 0,
}));
const label = computed(() =>
  indicator.value?.paused === true
    ? t('shell.recorder.label.paused', args.value)
    : t('shell.recorder.label.recording', args.value),
);
const title = computed(() =>
  indicator.value?.paused === true
    ? t('shell.recorder.title.paused', args.value)
    : t('shell.recorder.title.recording', args.value),
);
const stopTitle = computed(() => t('shell.recorder.stop', args.value));

function pause(): void {
  void shell.send({ type: 'runCommand', id: 'recorder.pause' });
}

function stop(): void {
  void shell.send({ type: 'runCommand', id: 'recorder.stop' });
}
</script>

<template>
  <div
    v-if="indicator !== null"
    role="group"
    :aria-label="title"
    class="flex shrink-0 items-center gap-0.5"
    data-testid="recorder-indicator"
  >
    <UButton
      :icon="indicator.paused ? ICONS.recordPaused : ICONS.record"
      :color="indicator.paused ? 'neutral' : 'error'"
      variant="soft"
      size="sm"
      class="max-w-40 truncate"
      :label="label"
      :title="title"
      data-testid="recorder-pause"
      @click="pause"
    />
    <UButton
      :icon="ICONS.recordStop"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :aria-label="stopTitle"
      :title="stopTitle"
      data-testid="recorder-stop"
      @click="stop"
    />
  </div>
</template>
