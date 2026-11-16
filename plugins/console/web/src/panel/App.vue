<script setup lang="ts">
// The console panel (RB-060-02) on Nuxt UI (RB-080-84). It asks `grouped` for the list every
// POLL_MS while visible, after a filter changed and after a clear, runs the command line through
// the command `exec` (a person's path, never MCP's) and shows the views as dots - shape by engine,
// label in the tooltip and for screen readers. The levels it starts with are `console.levels`,
// the command line's target `console.execTarget`; Enter runs, Shift+Enter breaks the line.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { highlighter } from '../../../../web-common/panel-ui/src/code/highlight';
import CodeEditor from '../../../../web-common/panel-ui/src/code/CodeEditor.vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { latestRefresh } from '../../../../web-common/panel-ui/src/refresh';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import EntryTable from './EntryTable.vue';
import Results from './Results.vue';
import Toolbar from './Toolbar.vue';
import { type ExecRecord, type ExecResult, type GroupedAnswer, groupedArgs, isCode, startLevels } from './model';

const POLL_MS = 1500;
const MAX_RESULTS = 20;
const panel = usePanel();
const t = panel.t;

const answer = ref<GroupedAnswer | null>(null);
const failure = ref<string | null>(null);
const levels = ref(new Set<string>());
const views = ref(new Set<number>());
const search = ref('');
const expression = ref('');
const target = ref<'all' | 'active'>('all');
const records = ref<ExecRecord[]>([]);
let started = false;
let nextRecord = 1;

const sentence = (error: unknown): string => {
  const code = error instanceof Error ? error.message : String(error);
  return isCode(code) ? panel.error(error) : code;
};

// One read at a time; a level, view or search change during a read reads again right after it,
// and the overtaken answer is not shown (review of RB-080-84).
const reader = latestRefresh(
  () => panel.call({ tool: 'grouped' }, groupedArgs({ levels: levels.value, views: views.value, search: search.value })),
  (value) => {
    answer.value = value as GroupedAnswer;
    failure.value = null;
  },
  (error) => {
    failure.value = sentence(error);
  },
);

function refresh(): Promise<void> {
  return started ? reader.refresh() : Promise.resolve();
}

/** The status line: a failure, no capture handed over, or the list cut to the newest. */
const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (failure.value) return { tone: 'error', text: failure.value };
  const now = answer.value;
  if (now && !now.captured) return { tone: 'info', text: t('panel.noCapture') };
  if (now?.truncated) return { tone: 'info', text: t('panel.truncated', { shown: now.shown, total: now.total }) };
  return null;
});

function toggleLevel(level: string): void {
  const next = new Set(levels.value);
  if (next.has(level)) next.delete(level);
  else next.add(level);
  levels.value = next;
  void refresh();
}

function toggleView(view: number): void {
  const next = new Set(views.value);
  if (next.has(view)) next.delete(view);
  else next.add(view);
  views.value = next;
  void refresh();
}

watch(search, () => void refresh());

function clear(): void {
  panel.call({ command: 'clear' }).then(
    () => refresh(),
    () => refresh(),
  );
}

async function run(): Promise<void> {
  const text = expression.value.trim();
  if (!text) return;
  const record: ExecRecord = { id: nextRecord++, expression: text, results: [] };
  try {
    const outcome = (await panel.call({ command: 'exec' }, { expression: text, target: target.value })) as { results?: ExecResult[] };
    record.results = outcome.results ?? [];
    expression.value = '';
  } catch (error) {
    record.error = t('panel.resultError', { text: sentence(error) });
  }
  records.value = [...records.value, record].slice(-MAX_RESULTS);
}

/** Enter runs the command; Shift+Enter breaks the line. */
function onCommandKey(event: KeyboardEvent): void {
  if (event.key !== 'Enter' || event.shiftKey || !(event.target instanceof HTMLTextAreaElement)) return;
  event.preventDefault();
  void run();
}

const targets = computed(() => [
  { label: t('panel.allViews'), value: 'all' },
  { label: t('panel.activeView'), value: 'active' },
]);

panel.onSettings((first) => {
  highlighter().configure(panel.settings.value);
  if (first || !started) {
    levels.value = new Set(startLevels(panel.settings.value));
    target.value = panel.settings.value['console.execTarget'] === 'active' ? 'active' : 'all';
  }
  started = true;
  void refresh();
});

const stopEvents = onPanelEvent((message) => {
  if (message && typeof message === 'object' && 'cleared' in message) void refresh();
});
const timer = setInterval(() => {
  if (!document.hidden && started) void reader.poll();
}, POLL_MS);
// The hint is the command field's description (CodeEditor names only its label).
onMounted(() => document.getElementById('expression')?.setAttribute('aria-describedby', 'expression-hint'));
onBeforeUnmount(() => {
  clearInterval(timer);
  stopEvents();
});
</script>

<template>
  <PanelFrame :title="t('panels.console')" icon="i-lucide-terminal" :status="line">
    <template #actions>
      <IconAction id="clear" icon="i-lucide-eraser" :label="t('panel.clear')" @click="clear" />
    </template>

    <div class="flex h-full min-h-0 flex-col gap-2">
      <Toolbar v-model:search="search" :answer="answer" :levels="levels" :views="views" @level="toggleLevel" @view="toggleView" />
      <EntryTable :entries="answer?.entries ?? []" :views="answer?.views ?? []" />
      <Results :records="records" :views="answer?.views ?? []" />
      <footer class="flex flex-wrap items-end gap-2 border-t border-default pt-2" @keydown="onCommandKey">
        <div class="flex min-w-48 flex-1 flex-col gap-1">
          <label id="expression-label" for="expression" class="text-xs font-medium text-muted">{{ t('panel.command') }}</label>
          <CodeEditor id="expression" v-model="expression" lang="js" :rows="1" :label="t('panel.command')" />
          <span id="expression-hint" class="sr-only">{{ t('panel.commandHint') }}</span>
        </div>
        <PanelTabs id="target" v-model="target" :items="targets" :content="false" size="xs" class="w-auto" :aria-label="t('panel.target')" />
        <UButton id="run" icon="i-lucide-play" :label="t('panel.run')" size="sm" @click="run" />
      </footer>
    </div>
  </PanelFrame>
</template>
