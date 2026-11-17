<script setup lang="ts">
// The toolbar's recording indicator (RB-050-01): shown only while a recording runs, in the
// toolbar strip (its own CEF view, never over a browser view). The status ("Recording (n)") is
// text; beside it a pause/resume button (`recorder.pause`, named by what it does) and a stop
// button with the visible word "Stop" (`recorder.stop`, saves; RB-090-60). Both are native,
// Tab-reachable buttons; the stop button's visible label is its name, the title its description
// (label in name, RB-090-23).
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import { ICONS } from '../icons';
import { useActivities } from '../activity/store';
import { useShell } from '../state/shell';
import { recorderIndicator } from './indicator';

const shell = useShell();
const activities = useActivities();
const { t } = useI18n();

// The compact form of the activity bar's recorder entry: gone while the entry is in the bar
// (RB-090-67).
const indicator = computed(() => (activities.inBar('recorder') ? null : recorderIndicator(shell.state.value)));
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
const pauseName = computed(() =>
  indicator.value?.paused === true
    ? t('shell.recorder.resume', args.value)
    : t('shell.recorder.pause', args.value),
);

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
    :aria-label="label"
    class="flex shrink-0 items-center gap-0.5"
    data-testid="recorder-indicator"
  >
    <UBadge
      :icon="indicator.paused ? ICONS.recordPaused : ICONS.record"
      :color="indicator.paused ? 'neutral' : 'error'"
      variant="soft"
      size="md"
      class="max-w-40 truncate"
      :label="label"
      data-testid="recorder-status"
    />
    <UButton
      :icon="indicator.paused ? ICONS.recordResume : ICONS.recordPaused"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :aria-label="pauseName"
      :title="title"
      data-testid="recorder-pause"
      @click="pause"
    />
    <UButton
      :icon="ICONS.recordStop"
      color="error"
      variant="soft"
      size="sm"
      :label="t('shell.recorder.stopLabel')"
      :title="stopTitle"
      data-testid="recorder-stop"
      @click="stop"
    />
  </div>
</template>
