<script setup lang="ts">
// The security header panel (manifest [[panels]], RB-093-03): the active view's page in each of
// the project's environments, side by side - a table of header x environment, a line that
// differs marked with a word and an icon, never by colour alone; "Only differences" hides the
// rest. Below, each environment's findings, or why its page could not be read. "Compare again"
// calls the tool `compare`; the command `compare_environments` posts `compare` to start it too.
// Too few environments links to Settings > Project > Environments (RB-094-21).
import { computed, h, onBeforeUnmount, ref } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import { type Comparison, failureText, findingText, type Line, lines, readEnvironments, sortedFindings, statusOf, text } from './model';

const SEVERITY: Record<string, { icon: string; class: string }> = {
  error: { icon: 'i-lucide-circle-x', class: 'text-error' },
  warning: { icon: 'i-lucide-triangle-alert', class: 'text-warning' },
  info: { icon: 'i-lucide-info', class: 'text-info' },
};

const panel = usePanel();
const strings = panel.strings;
const tx = (key: string, params?: Record<string, string | number>): string => text(strings.value, key, params);

const comparison = ref<Comparison | null>(null);
const loading = ref(false);
const failure = ref<string | null>(null);
/** The failed call's code: too few environments links to them (RB-094-21). */
const failureCode = ref<string | null>(null);
const ENVIRONMENTS_FAILURE = 'plugin.security-headers.errors.environments';
const onlyDifferences = ref(false);

const shown = computed<Line[]>(() => (comparison.value ? lines(strings.value, comparison.value, onlyDifferences.value) : []));
const line = computed(() => {
  if (panel.waiting.value) return { tone: 'busy' as const, text: strings.value['plugins.panel.waiting'] ?? '' };
  const now = statusOf(strings.value, { failure: failure.value, loading: loading.value, comparison: comparison.value });
  if (now && failure.value && failureCode.value === ENVIRONMENTS_FAILURE) {
    return { ...now, setting: { key: 'environments', label: tx('panel.setting_link') } };
  }
  return now;
});

const columns = computed<TableColumn<Line>[]>(() => [
  { id: 'header', header: tx('panel.header'), meta: { class: { td: 'align-top' } } },
  // A column per environment whose page was read; a failed one says why below, no empty column.
  ...(comparison.value ? readEnvironments(comparison.value) : []).map<TableColumn<Line>>((environment) => ({
    id: `env-${environment.id}`,
    header: environment.id,
    cell: ({ row }) => {
      const cell = row.original.cells[environment.id];
      if (!cell) return null;
      return cell.sent ? h('code', { class: 'text-xs break-all' }, cell.text) : h('span', { class: 'text-xs text-muted italic' }, cell.text);
    },
    meta: { class: { td: 'align-top max-w-64 whitespace-normal' } },
  })),
]);

function load(): void {
  loading.value = true;
  failure.value = null;
  failureCode.value = null;
  panel
    .call({ tool: 'compare' }, {})
    .then(
      (value) => {
        comparison.value = value as Comparison;
      },
      (error: unknown) => {
        comparison.value = null;
        failure.value = panel.error(error);
        failureCode.value = error instanceof Error ? error.message : null;
      },
    )
    .finally(() => {
      loading.value = false;
    });
}

// The command `compare_environments` posts `compare`.
const stopEvents = onPanelEvent((message) => {
  if ((message as { type?: unknown } | null)?.type === 'compare' && !loading.value) load();
});
onBeforeUnmount(stopEvents);

panel.onSettings((first) => {
  document.title = tx('panels.security-headers');
  if (first) load();
});
</script>

<template>
  <PanelFrame :title="tx('panels.security-headers')" icon="i-lucide-shield-check" :status="line">
    <template #actions>
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="tx('panel.refresh')" primary :disabled="loading" @click="load" />
    </template>

    <div v-if="comparison" class="flex flex-col gap-3">
      <div class="flex flex-wrap items-center justify-between gap-2 text-sm text-muted">
        <span id="path" class="break-all">{{ tx('panel.path', { path: comparison.path }) }}</span>
        <USwitch id="only" v-model="onlyDifferences" size="sm" :label="tx('panel.onlyDifferences')" />
      </div>

      <UTable id="table" :data="shown" :columns="columns" class="text-sm">
        <template #header-cell="{ row }">
          <div class="flex flex-col items-start gap-1" :data-line="row.original.key">
            <span class="font-mono text-xs break-all">{{ row.original.label }}</span>
            <UBadge v-if="row.original.differs" class="differs" color="warning" variant="subtle" size="sm" icon="i-lucide-diff" :label="tx('panel.differs')" />
          </div>
        </template>
      </UTable>

      <section v-for="environment in comparison.environments" :key="environment.id" :data-environment="environment.id" class="flex flex-col gap-1">
        <h2 class="flex items-center gap-2 text-sm font-semibold text-highlighted">
          <span>{{ environment.id }}</span>
          <span v-if="!environment.error" class="text-xs font-normal text-muted">{{ tx('panel.findings', { count: environment.findings?.length ?? 0 }) }}</span>
        </h2>
        <UAlert v-if="environment.error" color="warning" variant="subtle" icon="i-lucide-triangle-alert" :title="failureText(strings, environment.error)" :description="environment.url" />
        <ul v-else class="flex flex-col gap-1">
          <li v-for="(finding, index) in sortedFindings(environment)" :key="index" class="flex items-start gap-1.5 text-sm" :class="`sev-${finding.severity}`">
            <UIcon :name="SEVERITY[finding.severity]?.icon ?? 'i-lucide-info'" class="mt-0.5 size-4 shrink-0" :class="SEVERITY[finding.severity]?.class ?? 'text-info'" />
            <span class="sr-only">{{ tx(`panel.severity.${finding.severity}`) }}</span>
            <span>{{ findingText(strings, finding) }}</span>
          </li>
        </ul>
      </section>
    </div>
  </PanelFrame>
</template>
