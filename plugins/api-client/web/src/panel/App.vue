<script setup lang="ts">
// The API client panel (RB-096-01, laid out by RB-096-11) on Nuxt UI. Every action is an op the
// core runs through the plugin's `panel` tool (`{op: {...}}`). The side column is tabs -
// Requests (the collections' tree with its context menus, the last run, the recent sends),
// Variables (the environment file and the private values), Import, Export - under the
// environment picker, which is always there. The editor's header (method, URL, Send; title,
// collection, Save, Save as new, Delete, Copy as code, Benchmark) stands fixed above the
// request's tabs, so a tab switch moves nothing; the code, the answer and the benchmark follow
// below them. A send, a collection run or a benchmark to a host the project has not allowed fails
// with `host_not_allowed`; the panel then asks the person ("Allow <host> for this project?"), and
// only their Allow sends `allow` and runs it again (RB-096-13) - MCP has no `allow`. A
// `{{$secret name}}` without a value is said before the send, with "Set value". The command
// `send` posts `{send: true}`, `run_collection` `{runCollection: true}`; a request the network
// panel opened (RB-096-08) comes as the state's `draft` and waits over unsaved edits. A link to
// the private values from outside (RB-094-27) comes as the state's `tab`, the event `{tab: true}`
// asking an open panel for its state; a link inside the panel shows the tab itself.
import { computed, onBeforeUnmount, provide, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import BenchView from './BenchView.vue';
import { type CapturedDraft, draftNote, draftOf, editedSince, snapshotOf } from './captured';
import { ALL } from './code';
import CodeResult from './CodeResult.vue';
import { createCollections } from './collectionOps';
import { foldersWith } from './collections';
import ExportBar from './ExportBar.vue';
import ImportView from './ImportView.vue';
import type { ImportAnswer } from './importing';
import RecentList from './RecentList.vue';
import RequestEditor from './RequestEditor.vue';
import RequestHeader from './RequestHeader.vue';
import RequestTree from './RequestTree.vue';
import ResponseView from './ResponseView.vue';
import RunReport from './RunReport.vue';
import { createBenchRun, createCodeRun } from './runs';
import SecretsAlert from './SecretsAlert.vue';
import SettingsText from './SettingsText.vue';
import TreeDialog from './TreeDialog.vue';
import type { Report } from './checks';
import { type Answer, type ApiRequest, type HistoryEntry, type State, blockedHost, emptyRequest, folderOf, linkOf } from './model';
import { SHOW_SIDE_TAB, SIDE_TABS, isSideTab } from './sideTabs';
import VariableHints from './VariableHints.vue';
import VariablesView from './VariablesView.vue';
import { type SaveOp, type VariablesAnswer, blocksOf, references } from './variables';

const panel = usePanel();
const t = panel.t;

const state = ref<State | null>(null);
const request = ref<ApiRequest>(emptyRequest());
const id = ref<string | null>(null);
const folder = ref('');
const environment = ref<string | undefined>(undefined);
const answer = ref<Answer | null>(null);
const history = ref<HistoryEntry[]>([]);
/** The status line; `setting` is the place its text names (RB-094-21's `StatusLine.setting`). */
const note = ref<{ tone: Tone; text: string; setting?: { key: string; label: string } } | null>(null);
/** The host the person is asked to allow, and what runs again after their Allow. */
const asking = ref<{ host: string; kind: 'send' | 'run'; again: () => Promise<void> | void } | null>(null);
const busy = ref(false);
const report = ref<Report | null>(null);
const running = ref(false);
const side = ref<string>('requests');
const exportFolder = ref<string>(ALL);
const variables = ref<VariablesAnswer | null>(null);
const variablesProblem = ref<string | null>(null);
const missingSecrets = ref<string[]>([]);
const code = createCodeRun(panel);
const bench = createBenchRun(panel, (failure, again) => askFor(failure, 'run', again));
/** The editor content as last loaded, saved or emptied: what unsaved edits are measured against. */
const baseline = ref(snapshotOf(request.value));
const waitingDraft = ref<CapturedDraft | null>(null);
const waitingName = computed(() => {
  const draft = waitingDraft.value?.request;
  return draft ? draft.name || `${draft.method} ${draft.url}` : '';
});
const importEnvironments = computed(() => panel.settings.value['api-client.importEnvironments'] !== false);

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (panel.waiting.value) return { tone: 'busy', text: t('panel.waiting') };
  return note.value;
});
/** The side column's tabs as icons, each named by its label and tooltip, so the four fit a
 * narrow column without a scroll (RB-096-13). */
const sideTabs = computed(() =>
  SIDE_TABS.map(({ name, icon }) => ({ label: t(`panel.side.${name}`), icon, value: name, slot: name, ui: { label: 'sr-only', trigger: 'px-2.5' } })),
);
const environmentItems = computed(() => (state.value?.environments ?? []).map((name) => ({ label: name, value: name })));
const folders = computed(() => foldersWith(state.value?.requests ?? [], state.value?.folders ?? []));
const blocks = computed(() => (variables.value ? blocksOf(variables.value) : []));
const secretNames = computed(() => references(request.value).secrets);

async function op<T>(body: Record<string, unknown>): Promise<T> {
  return (await panel.call({ tool: 'panel' }, { op: body })) as T;
}

function fail(error: unknown): void {
  const link = linkOf(error);
  note.value = link ? { tone: 'error', text: panel.error(error), setting: { key: link.key, label: t(link.label) } } : { tone: 'error', text: panel.error(error) };
}

provide(SHOW_SIDE_TAB, (tab) => {
  side.value = tab;
});

/** Asks to allow the host of a `host_not_allowed` failure; `false` for any other failure. */
function askFor(failure: unknown, kind: 'send' | 'run', again: () => Promise<void> | void): boolean {
  const host = blockedHost(failure);
  if (host === null) return false;
  asking.value = { host, kind, again };
  note.value = null;
  return true;
}

function say(tone: Tone, text: string): void {
  note.value = { tone, text };
}

async function refresh(): Promise<void> {
  try {
    await readState();
    history.value = (await op<{ entries: HistoryEntry[] }>({ op: 'history' })).entries;
  } catch (error) {
    fail(error);
  }
  await loadVariables();
}

/** The state, with what it carries once: a request the network panel opened, the tab a link asked for. */
async function readState(): Promise<void> {
  const next = await op<State>({ op: 'state' });
  state.value = next;
  if (environment.value === undefined) environment.value = next.environment ?? undefined;
  const draft = draftOf(next);
  if (draft) offerDraft(draft);
  if (isSideTab(next.tab)) side.value = next.tab;
}

/** A link from outside asked for a tab: only the state is read, so unsaved variables stay. */
async function takeTab(): Promise<void> {
  try {
    await readState();
  } catch (error) {
    fail(error);
  }
}

async function loadVariables(): Promise<void> {
  try {
    variables.value = await op<VariablesAnswer>({ op: 'variables' });
    variablesProblem.value = null;
  } catch (error) {
    variablesProblem.value = panel.error(error);
  }
  await checkSecrets();
}

/** Which `{{$secret name}}`s of the request have no value in the environment. */
async function checkSecrets(): Promise<void> {
  const names = secretNames.value;
  if (names.length === 0) {
    missingSecrets.value = [];
    return;
  }
  try {
    missingSecrets.value = (await op<{ missing: string[] }>({ op: 'secrets', names, environment: environment.value ?? null })).missing;
  } catch {
    // The hint is a convenience: the send names a value still missing.
    missingSecrets.value = [];
  }
}
watch([() => secretNames.value.join('\n'), environment], () => void checkSecrets());

async function setSecret(name: string, value: string, scope: string | null): Promise<void> {
  try {
    await op({ op: 'setSecret', name, value, environment: scope });
    say('ok', t('panel.secretValue.done', { name }));
    await loadVariables();
  } catch (error) {
    fail(error);
  }
}

async function saveVariables(save: SaveOp): Promise<void> {
  try {
    await op({ ...save });
    say('ok', t('panel.variables.saved'));
    await refresh();
  } catch (error) {
    variablesProblem.value = panel.error(error);
  }
}

async function open(which: string): Promise<void> {
  try {
    const loaded = await op<{ id: string; request: ApiRequest }>({ op: 'load', id: which });
    request.value = loaded.request;
    id.value = loaded.id;
    folder.value = folderOf(loaded.id);
    settled();
    note.value = null;
  } catch (error) {
    fail(error);
  }
}

/** The editor content counts as unchanged from now on. */
function settled(): void {
  baseline.value = snapshotOf(request.value);
}

/** A request from the network panel: into the editor at once, or over unsaved edits once asked. */
function offerDraft(draft: CapturedDraft): void {
  if (editedSince(request.value, baseline.value)) waitingDraft.value = draft;
  else takeDraft(draft);
}

function replaceWithDraft(): void {
  const draft = waitingDraft.value;
  waitingDraft.value = null;
  if (draft) takeDraft(draft);
}

/** A request taken from the network panel: in the editor, not saved, in its host's folder. */
function takeDraft(draft: CapturedDraft): void {
  fresh();
  side.value = 'requests';
  request.value = draft.request;
  folder.value = draft.folder;
  settled();
  note.value = draftNote(draft, t);
}

function fresh(inFolder = ''): void {
  request.value = emptyRequest();
  settled();
  id.value = null;
  folder.value = inFolder;
  answer.value = null;
  note.value = null;
}

async function send(): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  asking.value = null;
  note.value = { tone: 'busy', text: t('panel.sending') };
  try {
    answer.value = await op<Answer>({ op: 'send', request: request.value, environment: environment.value ?? null, id: id.value });
    note.value = null;
  } catch (error) {
    if (!askFor(error, 'send', send)) fail(error);
  } finally {
    busy.value = false;
  }
  void refreshHistory();
}

async function refreshHistory(): Promise<void> {
  try {
    history.value = (await op<{ entries: HistoryEntry[] }>({ op: 'history' })).entries;
  } catch {
    // The history is a convenience: a failure to read it changes nothing else.
  }
}

/** The person's Allow: the host is kept for the project, then what asked runs again. */
async function allow(): Promise<void> {
  const asked = asking.value;
  if (!asked) return;
  try {
    await op({ op: 'allow', host: asked.host });
    asking.value = null;
    await asked.again();
    await refresh();
  } catch (error) {
    fail(error);
  }
}

async function save(asNew: boolean): Promise<void> {
  try {
    const saved = await op<{ id: string }>({
      op: 'save',
      id: asNew ? null : id.value,
      folder: asNew && id.value ? folderOf(id.value) || null : folder.value.trim() || null,
      request: request.value,
    });
    id.value = saved.id;
    settled();
    note.value = { tone: 'ok', text: t('panel.saved', { id: saved.id }) };
    await refresh();
  } catch (error) {
    fail(error);
  }
}

async function remove(): Promise<void> {
  const which = id.value;
  if (!which) return;
  try {
    await op({ op: 'delete', id: which });
    fresh();
    note.value = { tone: 'ok', text: t('panel.deleted', { id: which }) };
    await refresh();
  } catch (error) {
    fail(error);
  }
}

const collections = createCollections({
  op,
  t,
  state: () => state.value,
  refresh,
  say,
  fail,
  opened: () => id.value,
  open,
  freshIn: (inFolder) => fresh(inFolder),
  exportFolder: (which) => {
    exportFolder.value = which;
    side.value = 'export';
  },
  renamed: (next) => {
    if (next === null) fresh();
    else {
      id.value = next;
      folder.value = folderOf(next);
    }
  },
});

/** Every saved request sent and judged by its checks; the failures also go to the findings panel. */
async function runCollection(): Promise<void> {
  if (running.value) return;
  running.value = true;
  side.value = 'requests';
  note.value = { tone: 'busy', text: t('panel.run.running') };
  try {
    report.value = await op<Report>({ op: 'runCollection', environment: environment.value ?? null });
    note.value = null;
  } catch (error) {
    if (!askFor(error, 'run', runCollection)) fail(error);
  } finally {
    running.value = false;
  }
}

function fromHistory(entry: HistoryEntry): void {
  if (entry.id) {
    void open(entry.id);
    return;
  }
  fresh();
  request.value = { ...request.value, method: entry.method, url: entry.url };
  settled();
}

/** An import was written: the collections again, the count in the status line. */
async function imported(result: ImportAnswer): Promise<void> {
  await refresh();
  note.value = { tone: 'ok', text: t('panel.import.done', { count: result.requests.length - result.failed.length, folder: result.folder }) };
}

watch([id, environment], () => bench.reset(environment.value));

const stopEvents = onPanelEvent((message) => {
  if (message && typeof message === 'object' && (message as { send?: unknown }).send === true) void send();
  if (message && typeof message === 'object' && (message as { runCollection?: unknown }).runCollection === true) void runCollection();
  if (message && typeof message === 'object' && (message as { draft?: unknown }).draft === true) void refresh();
  if (message && typeof message === 'object' && (message as { tab?: unknown }).tab === true) void takeTab();
});

panel.onSettings((first) => {
  document.title = t('panels.api-client');
  if (first) void refresh();
});
onBeforeUnmount(stopEvents);
</script>

<template>
  <PanelFrame :title="t('panels.api-client')" icon="i-lucide-send" :status="line">
    <template #actions>
      <IconAction id="new" icon="i-lucide-file-plus" :label="t('panel.new')" @click="fresh()" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" @click="refresh" />
      <IconAction id="run-collection" icon="i-lucide-list-checks" :label="t('panel.run.start')" :disabled="running" @click="runCollection" />
    </template>

    <UAlert
      v-if="waitingDraft"
      id="draft-question"
      role="alertdialog"
      color="warning"
      variant="subtle"
      icon="i-lucide-triangle-alert"
      class="mb-3"
      :title="t('panel.capture.replace.title', { name: waitingName })"
      :description="t('panel.capture.replace.description')"
    >
      <template #actions>
        <UButton id="draft-replace" size="xs" :label="t('panel.capture.replace.yes')" @click="replaceWithDraft" />
        <UButton id="draft-keep" size="xs" color="neutral" variant="outline" :label="t('panel.capture.replace.no')" @click="waitingDraft = null" />
      </template>
    </UAlert>

    <div class="grid gap-3" :class="side === 'requests' ? 'md:grid-cols-[minmax(180px,1fr)_3fr]' : 'md:grid-cols-[minmax(280px,1fr)_1fr]'">
      <aside class="flex min-w-0 flex-col gap-2">
        <USelect
          id="environment"
          v-model="environment"
          :items="environmentItems"
          value-key="value"
          size="sm"
          icon="i-lucide-globe"
          class="w-full"
          :disabled="environmentItems.length === 0"
          :placeholder="t('panel.noEnvironment')"
          :aria-label="t('panel.environment')"
        />
        <TreeDialog v-if="collections.dialog.value" :dialog="collections.dialog.value" @answer="collections.answer" @cancel="collections.cancel" />
        <PanelTabs id="side-tabs" v-model="side" :items="sideTabs" :unmount-on-hide="false" variant="link" size="sm">
          <template #leading="{ item }">
            <UTooltip :text="item.label">
              <span data-side-icon class="inline-flex"><UIcon :name="item.icon ?? ''" class="size-4" /></span>
            </UTooltip>
          </template>
          <template #requests>
            <div class="flex flex-col gap-3 pt-2">
              <RequestTree :requests="state?.requests ?? []" :folders="folders" :selected="id" @act="collections.act" />
              <RunReport v-if="report" :report="report" @open="open" @close="report = null" />
              <RecentList :entries="history" @pick="fromHistory" />
            </div>
          </template>
          <template #variables>
            <VariablesView class="pt-2" :answer="variables" :problem="variablesProblem" :known="state?.environments ?? []" @save="saveVariables" @reload="loadVariables" />
          </template>
          <template #import>
            <ImportView :environments="importEnvironments" class="min-w-0 pt-2" @imported="imported" />
          </template>
          <template #export>
            <ExportBar v-model:folder="exportFolder" class="pt-2" :requests="state?.requests ?? []" />
          </template>
        </PanelTabs>
      </aside>

      <div class="flex min-w-0 flex-col gap-3">
        <RequestHeader
          v-model="request"
          v-model:folder="folder"
          :busy="busy"
          :id="id"
          :folders="folders"
          :environments="state?.environments ?? []"
          :environment="environment"
          :code="code"
          :bench="bench"
          @send="send"
          @save="save"
          @remove="remove"
        />
        <VariableHints :request="request" :environment="environment" :blocks="blocks" :base-urls="variables?.baseUrls ?? {}" :missing="missingSecrets" />
        <SecretsAlert :missing="missingSecrets" :environment="environment" @set="setSecret" />
        <UAlert
          v-if="asking"
          id="allow-question"
          role="alertdialog"
          color="warning"
          variant="subtle"
          icon="i-lucide-shield-question"
          :title="t('panel.allow.title', { host: asking?.host ?? '' })"
        >
          <template #description>
            <SettingsText text="panel.allow.description" setting-key="api-client.allowedHosts" />
          </template>
          <template #actions>
            <UButton id="allow" size="xs" :label="asking?.kind === 'send' ? t('panel.allow.yes') : t('panel.allow.yesRun')" @click="allow" />
            <UButton id="not-now" size="xs" color="neutral" variant="outline" :label="t('panel.allow.no')" @click="asking = null" />
          </template>
        </UAlert>

        <RequestEditor v-model="request" />

        <CodeResult :run="code" />
        <ResponseView v-if="answer" :answer="answer" />
        <BenchView v-if="id" :run="bench" />
      </div>
    </div>
  </PanelFrame>
</template>
