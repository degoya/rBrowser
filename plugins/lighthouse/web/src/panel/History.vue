<script setup lang="ts">
// The Lighthouse panel's history (RB-091-02): every run on disk, newest first, with its start
// time, its breakpoints and their performance score, an "Open report" per breakpoint and a
// delete that asks first, in place. The parent calls the plugin; this list only says what was
// chosen.
import { ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { runLabel, type HistoryRun } from './history';
import { rating } from './model';

defineProps<{ runs: HistoryRun[]; latest: string | null; loaded: boolean }>();
const emit = defineEmits<{ open: [run: string, breakpoint: string]; remove: [run: string] }>();

const t = usePanel().t;
const asking = ref<string | null>(null);

const COLOR = { good: 'success', average: 'warning', poor: 'error' } as const;
const scoreColor = (score: number | null | undefined) => {
  const rated = rating(score);
  return rated ? COLOR[rated] : 'neutral';
};

function confirmRemove(run: string): void {
  asking.value = null;
  emit('remove', run);
}
</script>

<template>
  <section class="flex flex-col gap-1" aria-labelledby="history-title">
    <h2 id="history-title" class="flex items-center gap-1.5 text-xs font-semibold uppercase text-toned">
      <UIcon name="i-lucide-history" class="size-4" />
      <span>{{ t('panel.history.title') }}</span>
    </h2>
    <p v-if="loaded && runs.length === 0" id="history-empty" class="text-sm text-muted">{{ t('panel.history.empty') }}</p>
    <ul id="history" class="divide-y divide-default">
      <li v-for="run in runs" :key="run.runId" data-run :data-run-id="run.runId" class="flex flex-col gap-1.5 py-2">
        <div class="flex flex-wrap items-center gap-2">
          <span class="flex-1 text-sm font-medium text-highlighted" data-run-label>{{ runLabel(run.runId) }}</span>
          <UBadge v-if="run.runId === latest" color="primary" variant="subtle" size="sm" :label="t('panel.history.latest')" />
          <UBadge v-if="run.cancelled" color="warning" variant="subtle" size="sm" :label="t('panel.history.cancelled')" />
          <UButton
            v-if="asking !== run.runId"
            data-delete
            size="xs"
            color="neutral"
            variant="ghost"
            icon="i-lucide-trash-2"
            :aria-label="t('panel.history.delete', { run: runLabel(run.runId) })"
            :title="t('panel.history.delete', { run: runLabel(run.runId) })"
            @click="asking = run.runId"
          />
        </div>
        <div v-if="asking === run.runId" data-confirm role="group" :aria-label="t('panel.history.confirm', { run: runLabel(run.runId) })" class="flex flex-wrap items-center gap-2 rounded-md border border-error/40 bg-error/10 p-2">
          <span class="flex-1 text-sm">{{ t('panel.history.confirm', { run: runLabel(run.runId) }) }}</span>
          <UButton data-confirm-delete size="xs" color="error" icon="i-lucide-trash-2" :label="t('panel.history.deleteYes')" @click="confirmRemove(run.runId)" />
          <UButton data-confirm-keep size="xs" color="neutral" variant="outline" :label="t('panel.history.keep')" @click="asking = null" />
        </div>
        <ul class="flex flex-wrap gap-1.5">
          <li v-for="entry in run.breakpoints" :key="entry.breakpoint">
            <UButton
              data-open
              :data-breakpoint="entry.breakpoint"
              size="xs"
              variant="outline"
              color="neutral"
              icon="i-lucide-file-text"
              :aria-label="t('panel.history.open', { breakpoint: entry.breakpoint, run: runLabel(run.runId) })"
              @click="emit('open', run.runId, entry.breakpoint)"
            >
              <span>{{ entry.breakpoint }}</span>
              <UBadge
                v-if="typeof entry.scores.performance === 'number'"
                data-score
                :color="scoreColor(entry.scores.performance)"
                variant="soft"
                size="sm"
                :label="String(entry.scores.performance)"
              />
            </UButton>
          </li>
        </ul>
      </li>
    </ul>
  </section>
</template>
