<script setup lang="ts">
// The storage panel (RB-070-01) on Nuxt UI (RB-080-84). Every call goes to the command `panel`
// with an `op` - a person's path, so the panel sees and edits values whatever the MCP switch
// says. A tab per kind (UTabs), the filters by origin and name, the grid of entries by view, the
// editor of a cell under it and the pinned values. It reads the open tab again every
// `storage-panel.pollMs` while visible and whenever a view navigates or a write lands.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import EntryEditor from './EntryEditor.vue';
import PinList from './PinList.vue';
import StorageGrid from './StorageGrid.vue';
import { KINDS, type Kind, type StorageList, type StoragePin, type StorageRow, type StorageView, arrange, listArgs } from './model';
import { writeEntry } from './write';

const panel = usePanel();
const t = panel.t;
const call = (args: Record<string, unknown>): Promise<unknown> => panel.call({ command: 'panel' }, args);

const kind = ref<Kind>('cookies');
const origin = ref('');
const search = ref('');
const list = ref<StorageList | null>(null);
const failure = ref<string | null>(null);
/** The outcome of a Delete in the grid (the editor, closed, has no status line then). */
const note = ref<string | null>(null);
const pins = ref<StoragePin[]>([]);
const pinsUntrusted = ref(false);
const editing = ref<{ key: number; row?: StorageRow; view?: StorageView } | null>(null);
const grid = ref<InstanceType<typeof StorageGrid> | null>(null);
let opened = 0;
let timer: ReturnType<typeof setInterval> | undefined;

const tabs = computed(() => KINDS.map((name) => ({ label: t('panel.tabs.' + name), value: name })));
const columns = computed(() => arrange(list.value?.views ?? []).columns);

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (failure.value) return { tone: 'error', text: failure.value };
  if (note.value) return { tone: 'info', text: note.value };
  if (list.value?.truncated) return { tone: 'info', text: t('panel.truncated', { shown: list.value.shown, total: list.value.total }) };
  return null;
});

// One read at a time; a tab, filter or write during a read reads again right after it, and the
// overtaken answer is not shown (review of RB-080-84).
const reader = latestRefresh(
  () => call(listArgs(kind.value, { origin: origin.value, search: search.value })),
  (value) => {
    const hadFocus = grid.value?.focused() ?? false;
    list.value = value as StorageList;
    failure.value = null;
    grid.value?.clamp();
    if (hadFocus) grid.value?.restore();
  },
  (error) => {
    failure.value = t('panel.error', { text: panel.error(error) });
  },
);

function refresh(): Promise<void> {
  return panel.ready.value ? reader.refresh() : Promise.resolve();
}

async function loadPins(): Promise<void> {
  try {
    const answer = (await call({ op: 'pins' })) as { pins?: StoragePin[]; untrusted?: boolean } | null;
    pins.value = answer?.pins ?? [];
    pinsUntrusted.value = answer?.untrusted === true;
  } catch {
    pins.value = [];
    pinsUntrusted.value = false;
  }
}

function open(row?: StorageRow, view?: StorageView): void {
  note.value = null;
  opened += 1;
  editing.value = { key: opened, row, view };
}

function close(): void {
  editing.value = null;
  grid.value?.restore();
}

/** Delete on a cell: the value goes in that view, the editor stays as it is. */
async function remove(row: StorageRow, view: StorageView): Promise<void> {
  const form = { name: row.name, scope: row.scope, path: row.path ?? '/', value: '', expires: '', sameSite: '', httpOnly: false, secure: false, domain: row.scope };
  note.value = await writeEntry(panel, kind.value, 'delete', view.id, form);
  await refresh();
}

function setPins(next: StoragePin[]): void {
  pins.value = next;
}

function enable(id: string, enabled: boolean): void {
  call({ op: 'enable', id, enabled }).then(
    (answer) => setPins((answer as { pins?: StoragePin[] } | null)?.pins ?? []),
    () => void loadPins(),
  );
}

function unpin(id: string): void {
  call({ op: 'unpin', id }).then(
    (answer) => {
      setPins((answer as { pins?: StoragePin[] } | null)?.pins ?? []);
      void refresh();
    },
    () => void loadPins(),
  );
}

watch(kind, () => {
  grid.value?.reset();
  void refresh();
});
watch([origin, search], () => void refresh());

function schedule(): void {
  if (timer !== undefined) clearInterval(timer);
  const every = panel.settings.value['storage-panel.pollMs'];
  const ms = typeof every === 'number' && every >= 0 ? every : 2000;
  timer =
    ms > 0
      ? setInterval(() => {
          if (!document.hidden && panel.ready.value) void reader.poll();
        }, Math.max(ms, 250))
      : undefined;
}

const stopEvents = onPanelEvent((message) => {
  if ((message as { type?: unknown } | null)?.type === 'changed') void refresh();
});

panel.onSettings((first) => {
  highlighter().configure(panel.settings.value);
  document.title = t('panels.storage');
  schedule();
  void refresh();
  if (first) void loadPins();
});
onBeforeUnmount(() => {
  if (timer !== undefined) clearInterval(timer);
  stopEvents();
});
</script>

<template>
  <PanelFrame :title="t('panels.storage')" icon="i-lucide-database" :status="line">
    <template #actions>
      <IconAction id="add" icon="i-lucide-plus" :label="t('panel.add')" primary @click="open()" />
    </template>

    <div class="flex flex-col gap-3">
      <UTabs id="tabs" v-model="kind" :items="tabs" :content="false" size="sm" variant="link" :aria-label="t('panels.storage')" />
      <div class="flex flex-wrap items-center gap-2">
        <UInput
          id="filter-origin"
          v-model="origin"
          type="search"
          icon="i-lucide-globe"
          size="sm"
          :maxlength="300"
          autocomplete="off"
          :spellcheck="false"
          :placeholder="t('panel.filterOrigin')"
          :aria-label="t('panel.filterOrigin')"
          class="min-w-40 flex-1"
        />
        <UInput
          id="filter-key"
          v-model="search"
          type="search"
          icon="i-lucide-search"
          size="sm"
          :maxlength="200"
          autocomplete="off"
          :spellcheck="false"
          :placeholder="t('panel.filterKey')"
          :aria-label="t('panel.filterKey')"
          class="min-w-40 flex-1"
        />
      </div>

      <StorageGrid ref="grid" :list="list" @open="open" @remove="remove" />

      <EntryEditor
        v-if="editing"
        :key="editing.key"
        :kind="kind"
        :row="editing.row"
        :view="editing.view"
        :views="columns"
        :pins="pins"
        @close="close"
        @written="refresh"
        @pins="setPins"
      />

      <PinList :pins="pins" :untrusted="pinsUntrusted" @enable="enable" @remove="unpin" />
    </div>
  </PanelFrame>
</template>
