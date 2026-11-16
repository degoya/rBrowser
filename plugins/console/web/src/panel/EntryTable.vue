<script setup lang="ts">
// The messages (RB-060-02, on Nuxt UI since RB-080-84): a `UTable` that draws only the rows in
// sight (`virtualize`, TanStack Virtual - the plugin keeps up to `console.maxEntries`, 10000), one
// row per deduplicated message: its level as a badge with icon and word, the message (JSON in
// syntax colours), how often, the source, and a dot per view that reported it. A message with a
// stack opens it below (the row expands). The keyboard is a list's: one tab stop (the focused
// message), arrows, Home and End move, Enter or Space opens the stack; a click opens it too.
import { computed, h, nextTick, ref, watch } from 'vue';
import type { TableColumn } from '@nuxt/ui';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import CodeBlock from '../../../../web-common/panel-ui/src/code/CodeBlock.vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { type GroupEntry, type ViewEntry, moveFocus, sourceLabel } from './model';
import LevelBadge, { LEVEL_TINT } from './LevelBadge.vue';
import Tokens from './Tokens.vue';
import ViewDots from './ViewDots.vue';

/** A row's height in pixels, the virtualizer's first guess before it measures. */
const ROW_PX = 52;

const props = defineProps<{ entries: GroupEntry[]; views: ViewEntry[] }>();
const { t, settings } = usePanel();
const focus = ref(0);
const expanded = ref<Record<string, boolean>>({});
const table = ref<{ $el: HTMLElement } | null>(null);

const at = computed(() => Math.min(focus.value, Math.max(props.entries.length - 1, 0)));

const columns = computed<TableColumn<GroupEntry>[]>(() => [
  { id: 'level', header: t('panel.levels'), meta: { class: { td: 'align-top' } } },
  { id: 'message', header: t('panel.entries'), meta: { class: { td: 'max-w-0 w-full whitespace-normal' } } },
  { id: 'views', header: () => h('span', { class: 'sr-only' }, t('panel.views')), meta: { class: { td: 'align-top' } } },
]);

const meta = computed(() => ({ class: { tr: (row: { original: GroupEntry }) => LEVEL_TINT[row.original.level] ?? '' } }));

/** A message that is JSON in syntax colours; any other text as it is (RB-080-65). */
function messageLang(text: string): HighlightLang | null {
  void settings.value;
  return highlighter().detect(text) === 'json' ? 'json' : null;
}

function toggle(index: number): void {
  const entry = props.entries[index];
  if (!entry?.stack) return;
  focus.value = index;
  const next = { ...expanded.value };
  if (next[entry.id]) delete next[entry.id];
  else next[entry.id] = true;
  expanded.value = next;
}

/** The message of row `index`, scrolled into the drawn range first when it is not drawn. */
async function focusRow(index: number): Promise<void> {
  focus.value = index;
  const root = table.value?.$el;
  const find = () => root?.querySelector<HTMLElement>(`[data-entry="${index}"]`);
  if (!find() && root) {
    root.scrollTop = Math.max(0, index * ROW_PX - root.clientHeight / 2);
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  }
  await nextTick();
  find()?.focus();
}

function onKey(event: KeyboardEvent): void {
  const row = (event.target as HTMLElement).closest<HTMLElement>('[data-entry]');
  if (!row) return;
  const index = Number(row.dataset.entry);
  const moved = moveFocus(index, event.key, props.entries.length);
  if (moved !== null) {
    event.preventDefault();
    void focusRow(moved);
  } else if ((event.key === 'Enter' || event.key === ' ') && props.entries[index]?.stack) {
    event.preventDefault();
    toggle(index);
  }
}

function onClick(event: MouseEvent): void {
  const tr = (event.target as HTMLElement).closest('tr');
  const row = tr?.querySelector<HTMLElement>('[data-entry]');
  if (row) toggle(Number(row.dataset.entry));
}

// Opened stacks of messages that are gone close.
watch(
  () => props.entries,
  (now) => {
    const ids = new Set(now.map((entry) => entry.id));
    const kept = Object.fromEntries(Object.entries(expanded.value).filter(([id]) => ids.has(id)));
    if (Object.keys(kept).length !== Object.keys(expanded.value).length) expanded.value = kept;
  },
);
</script>

<template>
  <section aria-labelledby="entries-title" class="flex min-h-48 flex-1 flex-col" @keydown="onKey" @click="onClick">
    <h2 id="entries-title" class="sr-only">{{ t('panel.entries') }}</h2>
    <UEmpty v-if="entries.length === 0" id="empty" icon="i-lucide-terminal" :title="t('panel.empty')" variant="naked" />
    <UTable
      v-else
      id="entries"
      ref="table"
      v-model:expanded="expanded"
      :data="entries"
      :columns="columns"
      :meta="meta"
      :get-row-id="(entry: GroupEntry) => entry.id"
      :virtualize="{ estimateSize: ROW_PX, overscan: 12 }"
      sticky
      class="min-h-0 flex-1 rounded-md border border-default"
      :ui="{ th: 'px-2 py-1 text-xs', td: 'px-2 py-1.5 text-sm', tr: 'cursor-default' }"
    >
      <template #level-cell="{ row }">
        <LevelBadge :level="row.original.level" />
      </template>
      <template #message-cell="{ row }">
        <div
          :data-entry="row.index"
          :data-id="row.original.id"
          :tabindex="row.index === at ? 0 : -1"
          :role="row.original.stack ? 'button' : undefined"
          :aria-expanded="row.original.stack ? row.getIsExpanded() : undefined"
          class="flex min-w-0 flex-col gap-0.5 rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-primary"
          :class="row.original.stack ? 'cursor-pointer' : ''"
        >
          <span class="flex min-w-0 items-start gap-1.5">
            <UIcon v-if="row.original.stack" :name="row.getIsExpanded() ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'" class="mt-0.5 size-4 shrink-0 text-muted" />
            <Tokens data-message class="min-w-0 font-mono text-xs break-words whitespace-pre-wrap text-highlighted" :text="row.original.text" :lang="messageLang(row.original.text)" />
          </span>
          <span class="flex flex-wrap items-center gap-1.5 text-xs text-muted">
            <UBadge v-if="row.original.count > 1" data-count color="neutral" variant="outline" size="sm" :label="t('panel.count', { count: row.original.count })" />
            <span
              data-source
              :data-kind="row.original.kind === 'rbrowser' ? 'rbrowser' : undefined"
              :title="sourceLabel(t, row.original).title || undefined"
              class="min-w-0 truncate"
              :class="row.original.kind === 'rbrowser' ? 'italic' : ''"
            >{{ sourceLabel(t, row.original).text }}</span>
          </span>
        </div>
      </template>
      <template #views-cell="{ row }">
        <ViewDots :ids="row.original.views" :views="views" :per-view="row.original.perView" />
      </template>
      <template #expanded="{ row }">
        <CodeBlock v-if="row.original.stack" data-stack class="stack" :aria-label="t('panel.stack')" :text="row.original.stack" lang="stack" />
      </template>
    </UTable>
  </section>
</template>
