<script setup lang="ts">
// The load test panel (RB-096-07) on Nuxt UI. Every action is an op the core runs through the
// plugin's `panel` tool (`{op: {...}}`): `state`, `load`, `save`, `delete`, `preview`, `allow`,
// `start`, `stop`, `compare`. A run starts in two clicks of the person: Review saves the
// scenario and shows what the run will do (`preview`); a target that is not the project's own or
// this machine needs the person's explicit confirmation (`allow` with `confirmed`); Start sends
// back the summary shown, and the core starts only that. While it runs the core pushes its
// numbers every second (`{loadTest: snapshot}`); Stop here and in the activity bar end it at
// once. It takes no focus by itself: the sandboxed frame refuses focus without a user's action.
// Without a saved request the editor's hint opens the API client panel (`openApiClient`, the
// core runs its own command); the panel reads its state again whenever it gets the focus back,
// so a request saved there meanwhile is in the list (RB-096-09).
import { computed, onBeforeUnmount, ref } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import type { CallError } from '../../../../web-common/panel-ui/src/panel';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import LiveView from './LiveView.vue';
import ReportList from './ReportList.vue';
import ScenarioEditor from './ScenarioEditor.vue';
import StartSummary from './StartSummary.vue';
import { type Scenario, type Snapshot, type State, type Summary, HOST_NOT_CONFIRMED, cleaned, emptyScenario, failureSetting, snapshotOf } from './model';

const panel = usePanel();
const t = panel.t;

const state = ref<State | null>(null);
const id = ref<string | null>(null);
const scenario = ref<Scenario | null>(null);
const summary = ref<Summary | null>(null);
const snapshot = ref<Snapshot | null>(null);
const note = ref<StatusLine | null>(null);
const busy = ref(false);
const stopping = ref(false);
const confirming = ref(false);

const line = computed<StatusLine | null>(() => {
  void panel.strings.value;
  if (panel.waiting.value) return { tone: 'busy', text: t('panel.waiting') };
  return note.value;
});
const running = computed(() => snapshot.value?.running === true);
const limits = computed(() => state.value?.limits ?? { maxUsers: 50, maxRps: 200, maxDurationS: 300 });

async function op<T>(body: Record<string, unknown>): Promise<T> {
  return (await panel.call({ tool: 'panel' }, { op: body })) as T;
}

function fail(error: unknown): void {
  const line: StatusLine = { tone: 'error', text: panel.error(error) };
  // A failure that names a setting links there (RB-094-21).
  const key = failureSetting(error instanceof Error ? error.message : '', (error as CallError).params);
  note.value = key === undefined ? line : { ...line, setting: { key, label: t('panel.setting_link') } };
}

async function refresh(): Promise<void> {
  try {
    const next = await op<State>({ op: 'state' });
    state.value = next;
    if (next.run && (!snapshot.value || next.running)) snapshot.value = next.run;
  } catch (error) {
    fail(error);
  }
}

async function open(which: string): Promise<void> {
  try {
    const loaded = await op<{ id: string; scenario: Scenario }>({ op: 'load', id: which });
    scenario.value = { ...emptyScenario(), ...loaded.scenario, abort: { ...emptyScenario().abort, ...loaded.scenario.abort } };
    id.value = loaded.id;
    summary.value = null;
    confirming.value = false;
    note.value = null;
  } catch (error) {
    fail(error);
  }
}

function fresh(): void {
  scenario.value = emptyScenario(state.value?.requests[0]?.id);
  id.value = null;
  summary.value = null;
  confirming.value = false;
  note.value = null;
}

/** Saves the open scenario; its id, or null when it did not save. */
async function save(): Promise<string | null> {
  if (!scenario.value) return null;
  try {
    const saved = await op<{ id: string }>({ op: 'save', id: id.value, scenario: cleaned(scenario.value) });
    id.value = saved.id;
    note.value = { tone: 'ok', text: t('panel.saved', { id: saved.id }) };
    await refresh();
    return saved.id;
  } catch (error) {
    fail(error);
    return null;
  }
}

async function remove(): Promise<void> {
  const which = id.value;
  if (!which) return;
  try {
    await op({ op: 'delete', id: which });
    note.value = { tone: 'ok', text: t('panel.deleted', { id: which }) };
    scenario.value = null;
    id.value = null;
    confirming.value = false;
    await refresh();
  } catch (error) {
    fail(error);
  }
}

/** Saves, then shows what the run will do. */
async function review(): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  try {
    const saved = await save();
    if (!saved) return;
    summary.value = await op<Summary>({ op: 'preview', id: saved });
    note.value = null;
  } catch (error) {
    summary.value = null;
    fail(error);
  } finally {
    busy.value = false;
  }
}

/** The person's explicit confirmation that `host` is theirs or theirs to test. */
async function confirmHost(host: string): Promise<void> {
  const which = id.value;
  if (!which || busy.value) return;
  busy.value = true;
  try {
    await op({ op: 'allow', host, confirmed: true });
    summary.value = await op<Summary>({ op: 'preview', id: which });
  } catch (error) {
    fail(error);
  } finally {
    busy.value = false;
  }
}

/** The person's Start: exactly the summary shown. */
async function start(): Promise<void> {
  const which = id.value;
  const shown = summary.value;
  if (!which || !shown || busy.value) return;
  busy.value = true;
  try {
    await op({ op: 'start', id: which, confirm: shown.confirm });
    summary.value = null;
    note.value = { tone: 'busy', text: t('panel.started', { name: shown.name || which }) };
  } catch (error) {
    const code = error instanceof Error ? error.message : '';
    if (code === HOST_NOT_CONFIRMED && (error as CallError).params?.host) {
      summary.value = await op<Summary>({ op: 'preview', id: which }).catch(() => null);
    }
    fail(error);
  } finally {
    busy.value = false;
  }
}

/** The API client panel, where the saved requests are made: the core opens it. */
async function openApiClient(): Promise<void> {
  try {
    await op({ op: 'openApiClient' });
  } catch (error) {
    fail(error);
  }
}

/** Back from another panel or window: a request saved there meanwhile is listed. */
function onFocus(): void {
  if (state.value !== null) void refresh();
}
window.addEventListener('focus', onFocus);

async function stop(): Promise<void> {
  if (stopping.value) return;
  stopping.value = true;
  try {
    await op({ op: 'stop' });
  } catch (error) {
    fail(error);
  } finally {
    stopping.value = false;
  }
}

const stopEvents = onPanelEvent((message) => {
  const next = snapshotOf(message);
  if (!next) return;
  const ended = snapshot.value?.running === true && !next.running;
  snapshot.value = next;
  if (ended) {
    note.value = { tone: 'ok', text: t(`panel.live.end.${next.end ?? 'completed'}`) };
    void refresh();
  }
});

panel.onSettings((first) => {
  document.title = t('panels.load-test');
  if (first) void refresh();
});
onBeforeUnmount(() => {
  stopEvents();
  window.removeEventListener('focus', onFocus);
});
</script>

<template>
  <PanelFrame :title="t('panels.load-test')" icon="i-lucide-activity" :status="line">
    <template #actions>
      <IconAction id="new" icon="i-lucide-file-plus" :label="t('panel.new')" @click="fresh" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" @click="refresh" />
    </template>

    <p id="own-systems" class="mb-3 text-xs text-muted">{{ t('panel.ownSystems') }}</p>

    <div class="grid gap-3 md:grid-cols-[minmax(180px,1fr)_3fr]">
      <div class="flex min-w-0 flex-col gap-3">
        <section id="scenarios" class="flex flex-col gap-0.5" :aria-label="t('panel.scenarios')">
          <p class="text-xs text-muted">{{ t('panel.scenarios') }}</p>
          <p v-if="(state?.scenarios ?? []).length === 0" id="scenarios-empty" class="text-xs text-muted">{{ t('panel.noScenarios') }}</p>
          <UButton
            v-for="listed in state?.scenarios ?? []"
            :key="listed.id"
            :data-scenario="listed.id"
            size="xs"
            :color="listed.id === id ? 'primary' : 'neutral'"
            :variant="listed.id === id ? 'soft' : 'ghost'"
            :aria-pressed="listed.id === id"
            class="justify-start font-normal"
            @click="open(listed.id)"
          >
            <span class="min-w-0 truncate">{{ listed.name || listed.id }}</span>
            <span v-if="listed.problem" class="text-xs text-error">{{ t('panel.unreadable') }}</span>
          </UButton>
        </section>
        <ReportList :reports="state?.reports ?? []" />
      </div>

      <div class="flex min-w-0 flex-col gap-3">
        <LiveView v-if="snapshot" :snapshot="snapshot" :stopping="stopping" @stop="stop" />

        <template v-if="scenario">
          <ScenarioEditor
            v-model="scenario"
            :requests="state?.requests ?? []"
            :environments="state?.environments ?? []"
            :limits="limits"
            @open-api-client="openApiClient"
            @refresh="refresh"
          />
          <div id="actions" class="flex flex-wrap items-center gap-1.5">
            <UButton id="review" size="sm" icon="i-lucide-list-checks" :disabled="busy || running" :label="t('panel.review')" @click="review" />
            <UButton id="save" size="sm" color="neutral" variant="outline" icon="i-lucide-save" :label="t('panel.save')" @click="save" />
            <template v-if="id">
              <UButton v-if="!confirming" id="delete" size="sm" color="error" variant="ghost" icon="i-lucide-trash" :label="t('panel.delete')" @click="confirming = true" />
              <template v-else>
                <span id="confirm-delete" class="text-sm">{{ t('panel.confirmDelete', { id }) }}</span>
                <UButton id="keep" size="sm" color="neutral" variant="outline" :label="t('panel.keep')" @click="confirming = false" />
                <UButton id="delete-yes" size="sm" color="error" :label="t('panel.delete')" @click="remove" />
              </template>
            </template>
          </div>
          <StartSummary v-if="summary" :summary="summary" :busy="busy || running" @confirm="confirmHost" @start="start" @close="summary = null" />
        </template>
        <p v-else-if="!snapshot" id="pick" class="text-sm text-muted">{{ t('panel.pick') }}</p>
      </div>
    </div>
  </PanelFrame>
</template>
