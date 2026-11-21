<script setup lang="ts">
// A check run's progress per plugin (RB-080-98): one row per plugin - name, state as icon and
// word, views through, findings, and a badge per state only for the views worth naming
// (running, failed, skipped) - in a `UAccordion` that opens a plugin to every view, a reason as
// its sentence. The rows' order and sums are `progress.ts`; `FindingsProgress.vue` frames them.
// After a run an open plugin offers "Run <plugin> again" (`rerun`, RB-094-17).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAccordion from '@nuxt/ui/components/Accordion.vue';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UIcon from '@nuxt/ui/components/Icon.vue';
import { failureParams } from './findings';
import { FINDINGS_ICONS } from './icons';
import { STATE_ICONS, type AttentionGroup, type PluginProgress } from './progress';
import type { CheckJob, CheckJobState } from './types';

const props = defineProps<{ rows: PluginProgress[]; canRerun?: boolean }>();
const emit = defineEmits<{ rerun: [plugin: string] }>();
const { t } = useI18n();

/** The plugins open, by id, so an open row stays open while the rows move. */
const open = ref<string[]>([]);
const items = computed(() => props.rows.map((row) => ({ value: row.plugin, row })));

/** More views than this in one badge are counted, not named. */
const NAMED_VIEWS = 3;

function viewName(job: CheckJob): string {
  if (job.view === undefined) {
    return t('findings.progress.all_views');
  }
  return job.breakpoint ?? t('findings.view_id', { id: String(job.view) });
}

function badgeText(group: AttentionGroup): string {
  const views =
    group.jobs.length > NAMED_VIEWS
      ? t('findings.progress.views_count', { count: group.jobs.length })
      : group.jobs.map(viewName).join(', ');
  return t('findings.progress.badge', { state: t(`findings.progress.state.${group.state}`), views });
}

function findingsText(row: PluginProgress): string {
  const through = row.jobs.some((job) => job.state === 'done');
  return through ? t('findings.progress.state.done', { count: row.findings }) : '';
}

function stateText(job: CheckJob): string {
  if (job.state === 'done') {
    return t('findings.progress.state.done', { count: job.findings ?? 0 });
  }
  return t(`findings.progress.state.${job.state}`);
}

/** A reason's sentence with its parameters (`failureParams`): no `{service}` raw, the host's
 * names of the plugin last, the `reason` of the plugin's own code only (RB-094-14). */
function reasonText(job: CheckJob): string {
  if (job.reason === undefined) {
    return '';
  }
  return t(job.reason, failureParams(job.reason, job.plugin, job.reasonParams));
}

/** A plugin through without findings or anything to name stays in the background. */
function quiet(row: PluginProgress): boolean {
  return row.state === 'done' && row.findings === 0 && row.attention.length === 0;
}

const iconClass: Record<CheckJobState, string> = {
  queued: 'text-muted',
  running: 'text-primary motion-safe:animate-spin',
  done: 'text-success',
  failed: 'text-error',
  skipped: 'text-muted',
};

const wordClass: Record<CheckJobState, string> = {
  queued: 'text-muted',
  running: 'text-primary',
  done: '',
  failed: 'text-error',
  skipped: 'text-muted',
};

const badgeColor: Record<AttentionGroup['state'], 'primary' | 'error' | 'neutral'> = {
  running: 'primary',
  failed: 'error',
  skipped: 'neutral',
};

const GRID = 'grid w-full items-center gap-x-3 gap-y-1 text-start grid-cols-[minmax(0,12rem)_6rem_5.5rem_7.5rem]';
</script>

<template>
  <UAccordion
    v-model="open"
    type="multiple"
    :items="items"
    :aria-label="t('findings.progress.plugins_label')"
    :ui="{
      root: '@container',
      trigger: 'gap-2 py-1 text-sm font-normal',
      label: 'min-w-0 flex-1',
      trailingIcon: 'size-4',
      body: 'pb-2 pt-0 text-sm',
    }"
  >
    <template #default="{ item }">
      <span
        :class="[GRID, '@2xl:grid-cols-[minmax(0,12rem)_6rem_5.5rem_7.5rem_minmax(0,1fr)]', quiet(item.row) ? 'text-muted' : '']"
        :data-testid="`progress-${item.row.plugin}`"
        :data-plugin="item.row.plugin"
      >
        <span class="truncate font-medium">{{ item.row.plugin }}</span>
        <span class="inline-flex items-center gap-1" :class="wordClass[item.row.state]">
          <UIcon :name="STATE_ICONS[item.row.state]" class="size-4 shrink-0" :class="iconClass[item.row.state]" aria-hidden="true" />
          {{ t(`findings.progress.plugin.${item.row.state}`) }}
        </span>
        <span class="tabular-nums">
          {{ t('findings.progress.views_through', { done: String(item.row.ended), total: item.row.total }) }}
        </span>
        <span class="tabular-nums">{{ findingsText(item.row) }}</span>
        <span v-if="item.row.attention.length > 0" class="col-span-full flex flex-wrap gap-1 @2xl:col-span-1">
          <UBadge
            v-for="group in item.row.attention"
            :key="group.state"
            data-testid="attention"
            size="sm"
            variant="subtle"
            :color="badgeColor[group.state]"
            :icon="STATE_ICONS[group.state]"
            :label="badgeText(group)"
            :ui="{ leadingIcon: group.state === 'running' ? 'motion-safe:animate-spin' : '' }"
          />
        </span>
      </span>
    </template>
    <template #body="{ item }">
      <ul class="flex flex-col gap-0.5 pl-6" :data-testid="`views-${item.row.plugin}`">
        <li
          v-for="job in item.row.jobs"
          :key="`${job.plugin}-${job.view ?? 'all'}`"
          class="grid grid-cols-[minmax(0,8rem)_minmax(0,1fr)] items-start gap-x-3"
          :class="job.state === 'failed' ? 'text-error' : job.state === 'done' && !job.findings ? 'text-muted' : ''"
          :data-view="viewName(job)"
        >
          <span class="truncate">{{ viewName(job) }}</span>
          <span class="flex flex-wrap items-center gap-x-1">
            <UIcon :name="STATE_ICONS[job.state]" class="size-4 shrink-0" :class="iconClass[job.state]" aria-hidden="true" />
            <span>{{ stateText(job) }}</span>
            <span v-if="job.reason !== undefined" :class="job.state === 'failed' ? '' : 'text-muted'">
              &middot; {{ reasonText(job) }}
            </span>
          </span>
        </li>
      </ul>
      <!-- A secondary action: an icon, named and explained by its tooltip. -->
      <UButton
        v-if="canRerun"
        size="sm"
        color="neutral"
        variant="ghost"
        class="ml-5"
        :icon="FINDINGS_ICONS.rerun"
        :aria-label="t('findings.rerun.one', { plugin: item.row.plugin })"
        :title="t('findings.rerun.one', { plugin: item.row.plugin })"
        :data-testid="`rerun-${item.row.plugin}`"
        @click="emit('rerun', item.row.plugin)"
      />
    </template>
  </UAccordion>
</template>
