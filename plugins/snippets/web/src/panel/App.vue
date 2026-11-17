<script setup lang="ts">
// The snippets panel (RB-070-04) on Nuxt UI (RB-080-84). It lists the snippets (`list`), edits
// and saves them (`save`) and runs them (`run`) view by view in parallel - each call answers
// within the shell's time - joining the views' results into one list. The host's events reload
// the list (`changed`) and show a run the command `run_last` made (`ran`). It takes no focus by
// itself: the sandboxed frame refuses focus without a user's action.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { errorText } from '../../../../web-common/panel-ui/src/format';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import RunResults from './RunResults.vue';
import SnippetEditor from './SnippetEditor.vue';
import SnippetList from './SnippetList.vue';
import { type ListAnswer, type RunAnswer, type RunRecord, type SnippetEntry, PREFIX, filter, join, runArgs, targetViews } from './model';

const MAX_RUNS = 20;
const panel = usePanel();
const t = panel.t;

const catalog = ref<ListAnswer | null>(null);
const selected = ref<SnippetEntry | null>(null);
const runs = ref<RunRecord[]>([]);
const note = ref<{ tone: Tone; text: string } | null>(null);
const query = ref('');
const name = ref('');
const code = ref('');
const target = ref<'all' | 'active'>('all');
const list = ref<InstanceType<typeof SnippetList> | null>(null);
const editor = ref<InstanceType<typeof SnippetEditor> | null>(null);
let nextRun = 1;

const shown = computed(() => filter(catalog.value?.snippets ?? [], query.value));

/** The status line: an action's outcome, else why some snippets are missing. */
const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (note.value) return note.value;
  const project = catalog.value?.project;
  const notes: string[] = [];
  if (project === 'untrusted') notes.push(t('panel.projectUntrusted'));
  if (project === 'disabled') notes.push(t('panel.projectDisabled'));
  if (project === 'noProject') notes.push(t('panel.noProject'));
  return notes.length ? { tone: 'info', text: notes.join(' ') } : null;
});

async function refresh(): Promise<void> {
  try {
    const answer = (await panel.call({ tool: 'list' }, { withCode: true })) as ListAnswer;
    catalog.value = answer;
    const was = selected.value;
    if (was) selected.value = answer.snippets.find((s) => s.source === was.source && s.name === was.name) ?? null;
  } catch (error) {
    note.value = { tone: 'error', text: panel.error(error) };
  }
}

function open(entry: SnippetEntry, run: boolean): void {
  selected.value = entry;
  name.value = entry.name;
  code.value = entry.code ?? '';
  target.value = entry.target;
  if (run) void nextTick(() => void runCode());
  else editor.value?.focusCode();
}

function addRun(title: string, results: RunRecord['results']): void {
  runs.value = [{ id: nextRun++, title, results }, ...runs.value].slice(0, MAX_RUNS);
}

async function runCode(): Promise<void> {
  const source = code.value;
  if (!source.trim()) return;
  const views = targetViews(target.value, catalog.value?.views ?? [], catalog.value?.active ?? null);
  if (views.length === 0) {
    note.value = { tone: 'error', text: errorText(panel.strings.value, PREFIX + 'errors.no_active_view') };
    return;
  }
  note.value = { tone: 'busy', text: t('panel.running', { count: views.length }) };
  const answers = await Promise.all(
    views.map((view) =>
      panel.call({ tool: 'run' }, runArgs(selected.value, source, view, views)).then(
        (answer) => ({ view, answer: answer as RunAnswer }),
        (error: unknown) => ({ view, error: error instanceof Error ? error.message : String(error) }),
      ),
    ),
  );
  note.value = null;
  addRun(name.value.trim() || t('panel.code'), join(answers));
}

async function save(): Promise<void> {
  const saved = name.value.trim();
  try {
    await panel.call({ tool: 'save' }, { name: saved, code: code.value });
    note.value = { tone: 'ok', text: t('panel.saved', { name: saved }) };
    selected.value = { source: 'local', name: saved, code: code.value, description: '', world: 'isolated', target: 'all', timeoutMs: 0 };
    await refresh();
  } catch (error) {
    note.value = { tone: 'error', text: panel.error(error) };
  }
}

async function remove(): Promise<void> {
  const entry = selected.value;
  if (entry?.source !== 'local') return;
  try {
    await panel.call({ tool: 'save' }, { name: entry.name, delete: true });
    selected.value = null;
    name.value = '';
    code.value = '';
    note.value = null;
    await refresh();
  } catch (error) {
    note.value = { tone: 'error', text: panel.error(error) };
  }
}

function fresh(): void {
  selected.value = null;
  name.value = '';
  code.value = '';
  editor.value?.focusName();
}

function onFilterKey(event: KeyboardEvent): void {
  const first = shown.value[0];
  if (event.key === 'Enter' && first) {
    event.preventDefault();
    open(first, false);
  } else if (event.key === 'ArrowDown') {
    event.preventDefault();
    list.value?.focusFirst();
  }
}

watch(query, () => list.value?.reset());
watch(
  () => panel.settings.value,
  (now) => highlighter().configure(now),
);
watch(
  () => panel.strings.value,
  () => {
    document.title = t('panels.snippets');
  },
);

const stopEvents = onPanelEvent((message) => {
  if (!message || typeof message !== 'object') return;
  const posted = message as { changed?: unknown; ran?: unknown };
  if ('changed' in posted) void refresh();
  if (posted.ran && typeof posted.ran === 'object') {
    const ran = posted.ran as RunAnswer;
    addRun(ran.snippet?.name ?? t('panel.code'), ran.results ?? []);
  }
});

panel.onSettings(() => void refresh());
onBeforeUnmount(stopEvents);
</script>

<template>
  <PanelFrame :title="t('panels.snippets')" icon="i-lucide-code" :status="line">
    <template #actions>
      <IconAction id="new" icon="i-lucide-file-plus" :label="t('panel.new')" @click="fresh" />
    </template>

    <div class="flex flex-col gap-3">
      <UInput
        id="filter"
        v-model="query"
        type="search"
        icon="i-lucide-search"
        size="sm"
        maxlength="200"
        autocomplete="off"
        spellcheck="false"
        :placeholder="t('panel.filter')"
        :aria-label="t('panel.filter')"
        class="w-full"
        @keydown="onFilterKey"
      />
      <div class="grid gap-3 md:grid-cols-[minmax(200px,2fr)_3fr]">
        <div class="min-w-0">
          <SnippetList ref="list" :entries="shown" :selected="selected" @open="open" />
          <UEmpty v-if="shown.length === 0" id="empty" icon="i-lucide-code" :title="t('panel.empty')" variant="naked" />
        </div>
        <SnippetEditor
          ref="editor"
          v-model:name="name"
          v-model:code="code"
          v-model:target="target"
          :selected="selected"
          :can-save="catalog?.local === 'loaded'"
          @run="runCode"
          @save="save"
          @remove="remove"
        />
      </div>
      <RunResults :runs="runs" :views="catalog?.views ?? []" />
    </div>
  </PanelFrame>
</template>
