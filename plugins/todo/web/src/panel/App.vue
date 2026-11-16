<script setup lang="ts">
// The todo panel (RB-110-13, docs/plugins.md): the project's .todo files in the Todo+ format as
// checklists. It lists them (`list`, in pages: load.ts), changes them through the tools
// (`toggle`, `add`, `edit`, `archive`) with the version it shows (`rev`) and this computer's UTC
// offset for the time stamps, and takes the file as it is now from every answer (its first page,
// the rest read on). A file that changed elsewhere is read again, never overwritten, and the
// status line says so. Every `todo.refreshSeconds` (no event tells a plugin about an editor's
// save) it looks at the heads of the files it lists and reads again those whose rev changed, and
// everything at once on Reload; a change over MCP arrives as `{ changed }`. The project is
// searched for its files only at the start and on Reload, and the window keeps the frame while
// another panel is shown (`keep`, RB-110-16), so a switch back shows the list at once.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import FileCard from './FileCard.vue';
import { loadFile, readFiles, type ListCall } from './load';
import { isConflict, panelSettings, readFailures, readListing, tagNames, utcOffset, type Done, type TodoFile } from './model';

const panel = usePanel();
const t = panel.t;

const files = ref<TodoFile[]>([]);
const truncated = ref(false);
const loaded = ref(false);
const busy = ref(false);
const editing = ref(0);
const tag = ref<string | null>(null);
const line = ref<StatusLine | null>(null);
const cards = ref<InstanceType<typeof FileCard>[]>([]);
const starterCard = ref<InstanceType<typeof FileCard> | null>(null);

const settings = computed(() => panelSettings(panel.settings.value));
const tags = computed(() => tagNames(files.value));
const tagItems = computed(() => tags.value.map((name) => ({ value: name, label: `@${name}` })));

const list: ListCall = (args) => panel.call({ tool: 'list' }, args);

/** Searches the folders again (`all`), or looks at the files shown only. */
let searchAll = true;
const reader = latestRefresh(
  () => {
    const all = searchAll || files.value.length === 0;
    searchAll = false;
    return readFiles(list, files.value, all);
  },
  (listing) => {
    files.value = listing.files;
    truncated.value = listing.truncated;
    loaded.value = true;
    if (line.value?.tone === 'error') line.value = null;
  },
  (error) => {
    loaded.value = true;
    line.value = { tone: 'error', text: panel.error(error) };
  },
);

function reload(): Promise<void> {
  searchAll = true;
  return reader.refresh();
}

/** Puts `file` in place of the file of the same path. */
function take(file: TodoFile): void {
  const at = files.value.findIndex((shown) => shown.path === file.path);
  if (at >= 0) files.value.splice(at, 1, file);
  else {
    files.value = [...files.value, file].sort((a, b) => a.path.localeCompare(b.path));
  }
}

async function act(file: TodoFile, tool: string, args: Record<string, unknown>, done?: Done): Promise<void> {
  busy.value = true;
  try {
    // A file not made yet has no version to hold the change to.
    const version = file.rev ? { rev: file.rev } : {};
    const answer = (await panel.call({ tool }, { file: file.path, ...version, utcOffset: utcOffset(), ...args })) as Record<string, unknown>;
    // The answer carries the file's first page; the rest is read on.
    if (answer.file) take(await loadFile(list, file.path, answer.file));
    done?.(answer);
    line.value = null;
  } catch (error) {
    if (isConflict(error)) {
      line.value = { tone: 'warn', text: t('panel.changed') };
      await reload();
    } else {
      line.value = { tone: 'error', text: panel.error(error) };
    }
  } finally {
    busy.value = false;
  }
}

/** Archives every file; a file that changed elsewhere is read again, any other that failed is
 * named with its reason. */
async function archive(): Promise<void> {
  busy.value = true;
  try {
    const answer = (await panel.call({ tool: 'archive' }, {})) as { archived?: unknown; files?: unknown; failed?: unknown; omitted?: unknown };
    // The heads of the files archived: their items are read again; heads the answer had no room
    // for are found by a look at every file shown.
    for (const head of readListing({ files: answer.files }).files) take(await loadFile(list, head.path));
    if (typeof answer.omitted === 'number' && answer.omitted > 0) await reader.refresh();
    const done = t('panel.archived', { n: typeof answer.archived === 'number' ? answer.archived : 0 });
    const failed = readFailures(answer.failed);
    const conflict = failed.some((failure) => isConflict(failure));
    const other = failed.find((failure) => !isConflict(failure));
    if (other) {
      // Read again first: a fresh list clears an error line, this one has to stay.
      if (conflict) await reload();
      line.value = { tone: 'error', text: `${done} ${panel.error(other)}` };
    } else if (conflict) {
      line.value = { tone: 'warn', text: t('panel.changed') };
      await reload();
    } else {
      line.value = { tone: 'ok', text: done };
    }
  } catch (error) {
    if (isConflict(error)) {
      line.value = { tone: 'warn', text: t('panel.changed') };
      await reload();
    } else {
      line.value = { tone: 'error', text: panel.error(error) };
    }
  } finally {
    busy.value = false;
  }
}

/** A new todo where it can go: the first file's field, or the first file made with one. */
function focusNew(): void {
  void (starterCard.value ?? cards.value[0])?.focusNew();
}

/** While a text is being edited the panel holds work: the panels window keeps it alive, and the
 * files are not read again under the editor's hands. */
function onEditing(on: boolean): void {
  editing.value = Math.max(0, editing.value + (on ? 1 : -1));
  panel.post({ type: 'dirty', dirty: editing.value > 0 });
}

const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object') return;
  const posted = message as { changed?: unknown; new?: unknown };
  if ('changed' in posted && !busy.value) {
    // A file the panel shows is read again; another one (made over MCP) needs the search.
    const known = typeof posted.changed !== 'string' || files.value.some((file) => file.path === posted.changed);
    void (known ? reader.poll() : reload());
  }
  if (posted.new === true) focusNew();
});

let timer: ReturnType<typeof setInterval> | null = null;
watch(
  () => [settings.value.refreshSeconds, panel.ready.value] as const,
  ([seconds, ready]) => {
    if (timer) clearInterval(timer);
    timer = null;
    if (ready && seconds > 0) {
      timer = setInterval(() => {
        if (!document.hidden && !busy.value && editing.value === 0) void reader.poll();
      }, seconds * 1000);
    }
  },
  { immediate: true },
);

// The window keeps this frame while another panel is shown (RB-110-16): the search for the
// files is not repeated on every switch back; the timer and Reload keep the list current.
panel.post({ type: 'keep' });

panel.onSettings((first) => {
  if (first) void reload();
});

watch(tags, (now) => {
  if (tag.value !== null && !now.includes(tag.value)) tag.value = null;
});

onBeforeUnmount(() => {
  stopEvents();
  if (timer) clearInterval(timer);
});

const status = computed<StatusLine | null>(() => {
  if (line.value) return line.value;
  if (!loaded.value) return { tone: 'busy', text: t('panel.loading') };
  if (truncated.value) return { tone: 'warn', text: t('panel.truncated') };
  return null;
});

/** No file yet: the field that starts `TODO.todo`. */
const starter = computed<TodoFile>(() => ({
  path: 'TODO.todo',
  rev: '',
  items: [],
  counts: { open: 0, done: 0, cancelled: 0 },
  error: null,
}));
</script>

<template>
  <PanelFrame :title="t('panels.todo')" icon="i-lucide-list-todo" :status="status">
    <template #actions>
      <IconAction id="archive" icon="i-lucide-archive" :label="t('panel.archive')" :disabled="busy || files.length === 0" @click="archive" />
      <IconAction id="reload" icon="i-lucide-refresh-cw" :label="t('panel.reload')" :disabled="busy" @click="reload" />
    </template>

    <div v-if="loaded" class="flex flex-col gap-4">
      <div v-if="tagItems.length > 0" class="flex items-center gap-2">
        <USelect
          id="tag"
          v-model="tag"
          :items="tagItems"
          size="sm"
          class="w-44"
          :placeholder="t('panel.filter')"
          :aria-label="t('panel.filter')"
        />
        <UButton v-if="tag !== null" id="tag-clear" size="xs" color="neutral" variant="ghost" icon="i-lucide-x" :aria-label="t('panel.filterClear')" @click="tag = null" />
      </div>

      <template v-if="files.length === 0">
        <UEmpty id="empty" icon="i-lucide-list-todo" :title="t('panel.empty')" variant="naked" />
        <FileCard ref="starterCard" :file="starter" :settings="settings" :tag="null" :busy="busy" @act="(tool, args, done) => act(starter, tool, args, done)" @editing="onEditing" />
      </template>
      <FileCard
        v-for="file in files"
        v-else
        :key="file.path"
        ref="cards"
        :file="file"
        :settings="settings"
        :tag="tag"
        :busy="busy"
        @act="(tool, args, done) => act(file, tool, args, done)"
        @editing="onEditing"
      />
    </div>
  </PanelFrame>
</template>
