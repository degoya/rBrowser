<script setup lang="ts">
// The Coverage panel (RB-080-07) on Nuxt UI (RB-080-84). It shows the last measurement (`get`
// with `mode: last`, asked every POLL_MS while visible and on every `refresh` event) as one row
// per file with one value - unused bytes and share with a bar, the smallest over the views, the
// span as a badge when they differ -, sorted by unused bytes; a row opens its values per
// breakpoint, and from there the file's detail (`file`). The views that could not be measured are
// named once, above the table. Filter by type, search by address, the total below. The header
// calls the commands `run_load`, `start`, `stop` and `export` (a load run's answer may come after
// the panels window stopped waiting; the next refresh shows it either way).
import { computed, h, onBeforeUnmount, ref } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import FileDetail from './FileDetail.vue';
import FileName from './FileName.vue';
import UnusedValue from './UnusedValue.vue';
import { type CoverageDetail, type CoverageSummary, type FileRow, type Kind, bytes, filterRows, measuredCount, percent, rows, totals, unmeasured } from './model';

const POLL_MS = 3000;
const STEP = 8000;
const panel = usePanel();
const t = panel.t;

const summary = ref<CoverageSummary | null>(null);
const failure = ref<string | null>(null);
const note = ref<{ tone: Tone; text: string } | null>(null);
/** The run the busy line waits for after the panels window stopped waiting (a long load run):
 * the start of the summary before it. The line goes when a newer summary is in or a poll fails. */
let waitingSince: { since: number | null } | null = null;
const detail = ref<CoverageDetail | null>(null);
const open = ref<{ url: string; kind: Kind; offset: number } | null>(null);
const kind = ref<Kind | 'all'>('all');
const query = ref('');
const expanded = ref<Record<string, boolean>>({});

const files = computed(() => rows(summary.value));
const shown = computed(() => filterRows(files.value, kind.value, query.value));
const sum = computed(() => totals(shown.value));
const missing = computed(() => unmeasured(summary.value));
const running = computed(() => summary.value?.running ?? false);

function done(): void {
  waitingSince = null;
  if (note.value?.tone === 'busy') note.value = null;
}

// One read at a time; a `refresh` event or a command's end during a read reads again right after
// it, and the overtaken answer is not shown (review of RB-080-84).
const reader = latestRefresh(
  () => panel.call({ tool: 'get' }, { mode: 'last' }),
  (value) => {
    const next = value as CoverageSummary;
    summary.value = next;
    failure.value = null;
    if (waitingSince && next.started !== waitingSince.since && !next.running) done();
  },
  (error) => {
    const code = error instanceof Error ? error.message : String(error);
    summary.value = null;
    failure.value = code.startsWith('plugin.coverage.errors.none') ? null : panel.error(error);
    if (waitingSince && failure.value) done();
  },
);

const refresh = (): Promise<void> => reader.refresh();

/** The status line: a failure, a command's outcome, the host's waiting, or the session running. */
const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (failure.value) return { tone: 'error', text: failure.value };
  if (note.value) return note.value;
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  if (running.value) return { tone: 'busy', text: t('panel.running') };
  return null;
});

/** What the table counts. */
const counted = computed(() => {
  const now = summary.value;
  if (!now) return '';
  return t('panel.summary', { mode: t('panel.mode.' + now.mode), views: measuredCount(now), files: files.value.length });
});

/** The views without a measurement, once: `mobile (Firefox)`, `tablet (failed: ...)`. */
const notMeasured = computed(() => {
  if (missing.value.length === 0) return '';
  const names = missing.value.map((view) =>
    view.status === 'unavailable'
      ? t('panel.unavailableView', { name: view.name, engine: engineName(view.engine) })
      : t('panel.failedView', { name: view.name, error: panel.error(view.error ?? '') }),
  );
  return t('panel.notMeasured', { views: names.join(', ') });
});

const ENGINES: Record<string, string> = { chromium: 'Chromium', firefox: 'Firefox', webkit: 'WebKit' };
const engineName = (engine: string): string => ENGINES[engine] ?? engine;

const filters = computed(() => {
  const count = (which: Kind | 'all') => filterRows(files.value, which, '').length;
  return [
    { label: t('panel.filter.all', { count: count('all') }), value: 'all' },
    { label: t('panel.filter.css', { count: count('css') }), value: 'css' },
    { label: t('panel.filter.js', { count: count('js') }), value: 'js' },
  ];
});

const columns = computed<TableColumn<FileRow>[]>(() => [
  { id: 'file', header: t('panel.file'), meta: { class: { td: 'max-w-0 w-full' } } },
  { id: 'unused', header: t('panel.unused'), meta: { class: { th: 'text-right', td: 'text-right' } } },
  { id: 'expand', header: () => h('span', { class: 'sr-only' }, t('panel.perView')), meta: { class: { td: 'w-px' } } },
]);

function toggle(row: FileRow): void {
  const key = String(row.id);
  expanded.value = expanded.value[key] ? {} : { [key]: true };
}

function showFile(url: string, which: Kind, offset: number): void {
  open.value = { url, kind: which, offset };
  panel.call({ tool: 'file' }, { url, kind: which, offset }).then(
    (value) => {
      detail.value = value as CoverageDetail;
    },
    (error: unknown) => {
      note.value = { tone: 'error', text: panel.error(error) };
    },
  );
}

function step(by: number): void {
  if (open.value) showFile(open.value.url, open.value.kind, Math.max(0, open.value.offset + by));
}

function close(): void {
  open.value = null;
  detail.value = null;
}

function command(id: string, doing?: string): void {
  failure.value = null;
  note.value = doing ? { tone: 'busy', text: t(doing) } : null;
  waitingSince = null;
  panel.call({ command: id }).then(
    (value) => {
      const written = (value as { written?: unknown } | null)?.written;
      note.value = Array.isArray(written) ? { tone: 'ok', text: t('panel.exported', { files: written.join(', ') }) } : null;
      void refresh();
    },
    (error: unknown) => {
      const code = error instanceof Error ? error.message : String(error);
      // The panels window stops waiting after its timeout; a load run goes on until a newer
      // summary is in.
      if (!code.startsWith('plugins.error.host_failed')) note.value = { tone: 'error', text: panel.error(error) };
      else if (note.value) waitingSince = { since: summary.value?.started ?? null };
      void refresh();
    },
  );
}

/** Arrow keys move between the rows of the table. */
function onKey(event: KeyboardEvent): void {
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
  const rowsOf = [...((event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('tbody tr[data-selectable="true"]'))];
  const at = rowsOf.indexOf(event.target as HTMLElement);
  if (at < 0) return;
  const target = rowsOf[event.key === 'ArrowDown' ? at + 1 : at - 1];
  if (!target) return;
  event.preventDefault();
  target.focus();
}

const stopEvents = onPanelEvent((raw) => {
  const message = raw as { exported?: unknown; refresh?: unknown } | null;
  const exported = message?.exported;
  if (Array.isArray(exported)) note.value = { tone: 'ok', text: t('panel.exported', { files: exported.join(', ') }) };
  if (message?.refresh) void refresh();
});
panel.onSettings(() => void refresh());
const timer = setInterval(() => {
  if (!document.hidden && panel.ready.value) void reader.poll();
}, POLL_MS);
onBeforeUnmount(() => {
  clearInterval(timer);
  stopEvents();
});
</script>

<template>
  <PanelFrame :title="t('panels.coverage')" icon="i-lucide-chart-pie" :status="line">
    <template #actions>
      <IconAction id="start" icon="i-lucide-circle-dot" :label="t('panel.start')" :disabled="running" @click="command('start')" />
      <IconAction id="stop" icon="i-lucide-circle-stop" :label="t('panel.stop')" :disabled="!running" @click="command('stop')" />
      <IconAction id="export" icon="i-lucide-download" :label="t('panel.export')" :disabled="!summary || running" @click="command('export')" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" @click="refresh" />
      <IconAction id="run-load" icon="i-lucide-play" :label="t('panel.runLoad')" primary @click="command('run_load', 'panel.measuring')" />
    </template>

    <div class="flex flex-col gap-3">
      <UEmpty v-if="!summary && !failure" id="empty" icon="i-lucide-chart-pie" :title="t('panel.idle')" variant="naked" />

      <template v-if="summary">
        <p id="summary" class="flex items-start gap-1.5 text-sm text-muted">
          <UIcon name="i-lucide-clock" class="mt-0.5 size-4 shrink-0" />
          <span>{{ counted }}</span>
        </p>
        <UAlert
          v-if="notMeasured"
          id="not-measured"
          color="neutral"
          variant="subtle"
          icon="i-lucide-circle-slash"
          :title="notMeasured"
          :ui="{ root: 'px-3 py-1.5', title: 'text-sm font-normal' }"
        />

        <div class="flex flex-wrap items-center gap-2">
          <PanelTabs id="kinds" v-model="kind" :items="filters" :content="false" size="xs" class="w-auto" />
          <UInput id="search" v-model="query" icon="i-lucide-search" size="sm" :placeholder="t('panel.search')" :aria-label="t('panel.search')" class="min-w-40 flex-1" />
        </div>

        <div @keydown="onKey">
          <UTable
            id="files"
            v-model:expanded="expanded"
            :data="shown"
            :columns="columns"
            :get-row-id="(row: FileRow) => String(row.id)"
            :caption="t('panel.caption')"
            :empty="t('panel.noMatch')"
            :on-select="(_: Event, row: { original: FileRow }) => toggle(row.original)"
            :ui="{ caption: 'sr-only', th: 'px-2 py-1.5', td: 'px-2 py-1.5', tr: 'cursor-pointer' }"
          >
            <template #file-cell="{ row }">
              <FileName :row="row.original" />
            </template>
            <template #unused-cell="{ row }">
              <UnusedValue :row="row.original" />
            </template>
            <template #expand-cell="{ row }">
              <UButton
                color="neutral"
                variant="ghost"
                size="xs"
                data-expand
                :icon="row.getIsExpanded() ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
                :aria-label="t(row.getIsExpanded() ? 'panel.collapse' : 'panel.expand')"
                :aria-expanded="row.getIsExpanded()"
                tabindex="-1"
                @click.stop="toggle(row.original)"
              />
            </template>
            <template #expanded="{ row }">
              <div data-views class="flex flex-col gap-1.5 py-1">
                <div v-for="value in row.original.views" :key="value.view" data-view class="grid grid-cols-[minmax(5rem,auto)_1fr_auto] items-center gap-2 text-xs">
                  <span class="truncate font-medium text-highlighted">{{ value.name }}</span>
                  <UProgress :model-value="value.percent" size="xs" color="warning" :aria-label="value.name" />
                  <span class="tabular-nums text-muted">{{ t('panel.viewValue', { size: bytes(value.unused), total: bytes(value.total), percent: percent(value.percent) }) }}</span>
                </div>
                <div>
                  <UButton
                    data-open
                    size="xs"
                    variant="link"
                    color="primary"
                    icon="i-lucide-file-code"
                    :label="t('panel.detail')"
                    @click.stop="showFile(row.original.url, row.original.measured, 0)"
                  />
                </div>
              </div>
            </template>
            <template v-if="shown.length > 0" #body-bottom>
              <tr id="totals" class="border-t border-default bg-elevated/50">
                <th scope="row" class="px-2 py-1.5 text-left text-sm font-semibold text-highlighted">{{ t('panel.total') }}</th>
                <td class="px-2 py-1.5 text-right text-sm whitespace-nowrap tabular-nums text-highlighted">
                  {{ t('panel.totalValue', { size: bytes(sum.unused), total: bytes(sum.total), percent: percent(sum.percent) }) }}
                </td>
                <td />
              </tr>
            </template>
          </UTable>
        </div>
        <p v-if="summary.truncated" id="truncated" class="text-xs text-muted">{{ t('panel.truncated') }}</p>
      </template>

      <FileDetail v-if="detail" :detail="detail" :summary="summary" @back="step(-STEP)" @more="step(STEP)" @close="close" />
    </div>
  </PanelFrame>
</template>
