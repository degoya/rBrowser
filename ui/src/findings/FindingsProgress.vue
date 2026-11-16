<script setup lang="ts">
// A check run while it goes (RB-080-89): the bar with "3 of 12 checks", the time it has run and,
// once a last run gave timings, about how long is left; Cancel; then one row per plugin, the
// running ones on top, each view on demand (`ProgressRows.vue`, RB-080-98). After the run it
// collapses to one line - the checks, the time, how many failed or were skipped - that opens the
// rows again. The logic is `progress.ts`; the panel (`FindingsPanel.vue`) feeds it and ticks
// `elapsedMs`. A screen reader hears the count and the end only (RB-090-22).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCollapsible from '@nuxt/ui/components/Collapsible.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import UProgress from '@nuxt/ui/components/Progress.vue';
import ProgressRows from './ProgressRows.vue';
import { FINDINGS_ICONS } from './icons';
import { STATE_ICONS, pluginRows, progressCounts, runTally, secondsLeft, wholeSeconds } from './progress';
import type { ChecksProgress } from './types';

const props = defineProps<{ progress: ChecksProgress; elapsedMs: number }>();
const emit = defineEmits<{ cancel: [] }>();
const { t } = useI18n();

const counts = computed(() => progressCounts(props.progress));
const rows = computed(() => pluginRows(props.progress));
const left = computed(() => secondsLeft(props.progress.estimateMs, props.elapsedMs));
const tally = computed(() => runTally(props.progress));
/** The summary line's icon: cancelled, failed in a view, or through. */
const outcome = computed(() => {
  if (props.progress.cancelled === true) {
    return { icon: STATE_ICONS.skipped, tone: 'text-muted' };
  }
  return tally.value.failed > 0
    ? { icon: STATE_ICONS.failed, tone: 'text-error' }
    : { icon: STATE_ICONS.done, tone: 'text-success' };
});
const summary = computed(() => {
  const seconds = String(wholeSeconds(props.elapsedMs));
  return props.progress.cancelled === true
    ? t('findings.progress.summary_cancelled', { done: String(tally.value.ran), total: tally.value.total, seconds })
    : t('findings.progress.summary', { total: tally.value.total, seconds });
});
/** What a screen reader hears (RB-090-22): the count while the run goes, its summary at the end -
 *  never the ticking time, so it speaks only when a check is through or the run ends. */
const announced = computed(() =>
  props.progress.finished
    ? summary.value
    : t('findings.progress.count', { done: String(counts.value.done), total: counts.value.total }),
);
/** The rows of a finished run, opened from its summary line. */
const details = ref(false);
</script>

<template>
  <!-- One live region for the whole run, kept while it turns into the summary. -->
  <p class="sr-only" role="status" data-testid="progress-status">{{ announced }}</p>
  <section
    v-if="!progress.finished"
    class="flex flex-col gap-2 text-sm"
    data-testid="progress"
    :aria-label="t('findings.progress.label')"
  >
    <div class="flex flex-wrap items-center gap-3">
      <UProgress class="min-w-32 flex-1" size="sm" :model-value="counts.done" :max="Math.max(counts.total, 1)" />
      <span class="tabular-nums">{{ t('findings.progress.count', { done: String(counts.done), total: counts.total }) }}</span>
      <span class="text-muted tabular-nums">{{ t('findings.progress.elapsed', { seconds: String(wholeSeconds(elapsedMs)) }) }}</span>
      <span v-if="left !== null" class="text-muted tabular-nums">{{ t('findings.progress.left', { seconds: String(left) }) }}</span>
      <UButton size="sm" variant="outline" color="neutral" data-testid="cancel" @click="emit('cancel')">
        {{ t('findings.cancel') }}
      </UButton>
    </div>
    <ProgressRows :rows="rows" />
  </section>
  <UCollapsible
    v-else
    v-model:open="details"
    class="flex flex-col gap-1 text-sm"
    data-testid="progress-summary"
    :aria-label="t('findings.progress.label')"
  >
    <UButton
      data-testid="progress-details"
      size="sm"
      variant="ghost"
      color="neutral"
      class="self-start"
      :trailing-icon="FINDINGS_ICONS.checkDetails"
      :ui="{ trailingIcon: details ? 'rotate-180 transition-transform' : 'transition-transform' }"
    >
      <UIcon :name="outcome.icon" class="size-4 shrink-0" :class="outcome.tone" aria-hidden="true" />
      <span class="tabular-nums">{{ summary }}</span>
      <UBadge
        v-if="tally.failed > 0"
        size="sm"
        variant="subtle"
        color="error"
        :icon="STATE_ICONS.failed"
        :label="t('findings.progress.failed_count', { count: String(tally.failed) })"
      />
      <UBadge
        v-if="tally.skipped > 0"
        size="sm"
        variant="subtle"
        color="neutral"
        :icon="STATE_ICONS.skipped"
        :label="t('findings.progress.skipped_count', { count: String(tally.skipped) })"
      />
      <span class="text-muted">{{ t('findings.progress.details') }}</span>
    </UButton>
    <template #content>
      <ProgressRows :rows="rows" />
    </template>
  </UCollapsible>
</template>
