<script setup lang="ts">
// The error report panel (RB-080-09) on Nuxt UI (RB-080-84). It asks `get` for the report every
// POLL_MS while visible and after the plugin's events, runs "Report now" through the command
// `run` (with the reload the checkbox chose, first from `error-report.reloadBeforeReport`) and
// clears through the command `clear`. The entries are listed by category, each with its severity
// as a badge in words, a dot per view that reported it and its count; a row opens to its source,
// request and stack (Enter, Space or a click; the arrows, Home and End move between the rows).
// At most LIMIT entries come per answer, so the list is drawn whole - no virtual scrolling.
import { computed, nextTick, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import EntryDetails from './EntryDetails.vue';
import ViewDots from './ViewDots.vue';
import { type ReportAnswer, type ReportEntry, byCategory, keyName, moveFocus, severityOf, viewColor } from './model';

const POLL_MS = 2000;
const LIMIT = 200;
const panel = usePanel();
const t = panel.t;

const answer = ref<ReportAnswer | null>(null);
const note = ref<{ tone: Tone; text: string } | null>(null);
const reload = ref(false);
const expanded = ref<Set<string>>(new Set());
const focusIndex = ref(0);
const list = ref<HTMLElement | null>(null);

const groups = computed(() => byCategory(answer.value?.entries ?? []));
/** The entries in the order they are drawn, for the keyboard. */
const order = computed<ReportEntry[]>(() => groups.value.flatMap((group) => group.entries));
const indexOf = (entry: ReportEntry): number => order.value.indexOf(entry);
const roving = computed(() => Math.min(focusIndex.value, Math.max(order.value.length - 1, 0)));

/** The status line: no capture, the list cut, or the last command's outcome. */
const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  const now = answer.value;
  if (now && !now.captured) return { tone: 'warn', text: t('panel.noCapture') };
  if (now?.truncated) return { tone: 'info', text: t('panel.truncated', { shown: now.entries.length, total: now.total }) };
  if (note.value) return note.value;
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  return null;
});

function showError(error: unknown): void {
  note.value = { tone: 'error', text: panel.error(error) };
}

// One read at a time; an event or an action during a read reads again right after it, and the
// overtaken answer is not shown (review of RB-080-84).
const reader = latestRefresh(
  () => panel.call({ tool: 'get' }, { limit: LIMIT }),
  (value) => {
    answer.value = value as ReportAnswer;
    if (answer.value.published) note.value = { tone: 'ok', text: t('panel.published') };
  },
  showError,
);

function refresh(): Promise<void> {
  return panel.ready.value ? reader.refresh() : Promise.resolve();
}

async function runNow(): Promise<void> {
  try {
    const outcome = ((await panel.call({ command: 'run' }, { reload: reload.value })) ?? {}) as { published?: unknown; reloaded?: unknown[] };
    note.value = { tone: 'ok', text: outcome.reloaded ? t('panel.reloaded', { count: outcome.reloaded.length }) : t('panel.published') };
  } catch (error) {
    showError(error);
    return;
  }
  await refresh();
}

function clear(): void {
  note.value = null;
  panel.call({ command: 'clear' }, {}).then(
    () => refresh(),
    (error: unknown) => showError(error),
  );
}

function toggle(entry: ReportEntry): void {
  const next = new Set(expanded.value);
  if (next.has(entry.id)) next.delete(entry.id);
  else next.add(entry.id);
  expanded.value = next;
}

function onClick(event: MouseEvent, entry: ReportEntry): void {
  if ((event.target as HTMLElement).closest('[data-details]')) return;
  focusIndex.value = indexOf(entry);
  toggle(entry);
}

function onKey(event: KeyboardEvent, entry: ReportEntry): void {
  if (event.target !== event.currentTarget) return;
  const index = indexOf(entry);
  const moved = moveFocus(index, event.key, order.value.length);
  if (moved !== null) {
    event.preventDefault();
    focusIndex.value = moved;
    void nextTick(() => list.value?.querySelectorAll<HTMLElement>('li[data-entry]')[moved]?.focus());
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    toggle(entry);
  }
}

const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object') return;
  const posted = message as { reloaded?: unknown; published?: unknown; cleared?: unknown };
  if (typeof posted.reloaded === 'number') note.value = { tone: 'ok', text: t('panel.reloaded', { count: posted.reloaded }) };
  else if (posted.published) note.value = { tone: 'ok', text: t('panel.published') };
  else if ('cleared' in posted) note.value = null;
  void refresh();
});

panel.onSettings((first) => {
  if (first) reload.value = panel.settings.value['error-report.reloadBeforeReport'] === true;
  void refresh();
});
const timer = setInterval(() => {
  if (!document.hidden && panel.ready.value) void reader.poll();
}, POLL_MS);
onBeforeUnmount(() => {
  clearInterval(timer);
  stopEvents();
});
</script>

<template>
  <PanelFrame :title="t('panels.report')" icon="i-lucide-bug" :status="line">
    <template #actions>
      <IconAction id="clear" icon="i-lucide-trash-2" :label="t('panel.clear')" @click="clear" />
      <IconAction id="run" icon="i-lucide-play" :label="t('panel.run')" primary @click="runNow" />
    </template>

    <div class="flex flex-col gap-3">
      <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
        <div v-if="answer && answer.views.length > 0" id="views" role="group" :aria-label="t('panel.views')" class="flex min-w-0 flex-1 flex-wrap gap-x-3 gap-y-1 text-xs text-muted">
          <UTooltip v-for="(view, index) in answer.views" :key="view.id" :text="view.breakpoint + ' - ' + view.engine">
            <span data-view class="inline-flex items-center gap-1.5">
              <span
                class="inline-block size-2.5 shrink-0"
                :class="view.engine === 'firefox' ? 'rounded-[1px]' : view.engine === 'webkit' ? 'rotate-45 scale-85 rounded-[1px]' : 'rounded-full'"
                :style="{ background: viewColor(index) }"
              />
              <span>{{ view.label }}</span>
            </span>
          </UTooltip>
        </div>

        <UCheckbox id="reload" v-model="reload" :label="t('panel.reload')" size="sm" class="ml-auto" />
      </div>

      <UEmpty v-if="order.length === 0" id="empty" icon="i-lucide-circle-check" :title="t('panel.empty')" variant="naked" />

      <div v-else id="categories" ref="list" role="group" :aria-label="t('panel.entries')" class="flex flex-col gap-3">
        <section v-for="group in groups" :key="group.category" data-category class="flex flex-col gap-1">
          <h2 class="sticky top-0 z-10 flex items-center gap-2 bg-default py-1 text-xs font-semibold uppercase text-toned">
            <span data-heading>{{ t('rules.' + keyName(group.category)) }}</span>
            <UBadge data-total color="neutral" variant="subtle" size="sm" :label="String(group.entries.length)" />
          </h2>
          <ol role="tree" :aria-label="t('rules.' + keyName(group.category))" class="divide-y divide-default overflow-hidden rounded-md border border-default">
            <li
              v-for="entry in group.entries"
              :key="entry.id"
              data-entry
              role="treeitem"
              :data-id="entry.id"
              :data-severity="entry.severity"
              :tabindex="indexOf(entry) === roving ? 0 : -1"
              :aria-expanded="expanded.has(entry.id)"
              class="cursor-pointer border-l-3 px-2 py-1.5 outline-none hover:bg-elevated/50 focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
              :class="entry.severity === 'error' ? 'border-l-error' : entry.severity === 'warning' ? 'border-l-warning' : 'border-l-info'"
              @click="onClick($event, entry)"
              @keydown="onKey($event, entry)"
            >
              <div class="flex flex-wrap items-center gap-2">
                <UBadge
                  data-severity-badge
                  :data-tone="severityOf(entry.severity).color"
                  :color="severityOf(entry.severity).color"
                  :icon="severityOf(entry.severity).icon"
                  variant="subtle"
                  size="sm"
                  :label="t('severities.' + entry.severity)"
                />
                <ViewDots :ids="entry.views" :per-view="entry.perView" :views="answer?.views ?? []" />
                <span data-message class="min-w-0 flex-[1_1_12em] text-sm break-words whitespace-pre-wrap text-highlighted">{{ entry.text }}</span>
                <UBadge v-if="entry.count > 1" data-count color="neutral" variant="outline" size="sm" :label="t('panel.count', { count: entry.count })" />
                <UBadge v-if="entry.onlyIn" data-only color="warning" variant="subtle" size="sm" :label="t('panel.onlyIn', { views: entry.onlyIn.join(', ') })" />
                <UIcon :name="expanded.has(entry.id) ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'" class="size-4 shrink-0 text-muted" aria-hidden="true" />
              </div>
              <EntryDetails v-if="expanded.has(entry.id)" :entry="entry" />
            </li>
          </ol>
        </section>
      </div>
    </div>
  </PanelFrame>
</template>
