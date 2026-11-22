<script setup lang="ts">
// The Recorder panel (RB-090-60, RB-050-01's step list), a core panel of the panels page
// (`panels/core.ts`, `panels.recorder`, Ctrl+Shift+Space F V): the project's recordings
// (`.rbrowser/out/recordings/`, newest first) and, for the one picked, its steps - kind,
// element, name, value and `onlyIn` - to rename, delete, move and change (`StepRow.vue`). The
// edits stay in the panel until "Save" writes the recording over its file (`saveRecording`, the
// core checks it against `recording.v1`); "Revert" reads it again. "Export" opens the export
// dialog (`ExportDialog.vue`); a save there says here where the files went. "Play" (RB-090-66)
// plays the chosen recording in its breakpoints' views through the flows runner
// (`recorder.play`), at the pace chosen beside it (`pace.ts`, starting at
// `automation.recorder.playbackPace`); it waits while a recording runs, while edits are unsaved
// and while a run goes on - which shows here with "Cancel" (`flows.cancel`; the core's
// `flowRunChanged` asks again). While no recording runs, "Start recording" starts one
// (`recorder.start`) and, while one runs, "Stop recording" in its place stops it (`recorder.stop`,
// RB-090-94); the core's `recordingsChanged` (a recording stopped or saved) refreshes the
// list, and a recording with unsaved edits is never replaced under the person's hands. While the
// chosen recording plays (RB-090-84) its steps follow the run (`playback.ts`): the step a view
// runs is marked, a failed one red with its reason, and with `automation.recorder.followPlayback`
// the running step scrolls into sight; each step shows the pause Play waits before it at the pace
// As recorded - the exporter's, answered with the recording, left out while edits are unsaved.
// While a recording runs (RB-090-89, `automation.recorder.showLiveSteps`) its steps show as they
// are taken (`LiveSteps.vue`; a panel opened meanwhile catches up); once it is saved the panel
// opens it unless edits are unsaved, and the setting switched off only hides the list. The list
// of recordings is `RecordingNav.vue`, with "Delete" beside each (RB-090-91, `recorder.delete`,
// asking first while `automation.recorder.confirmDelete` is on); a recording gone from the list
// closes here; another recording chosen over unsaved edits asks first, and the chosen one
// changed on disk is read again - or, over unsaved edits, waits for Revert (`RecordingBar.vue`,
// `draftGuard.ts`). The step list is `StepList.vue` with the recording editor (RB-092-01): steps added,
// changed, switched off, tried, played from, continued from - the edits stay in the draft until
// "Save"; "Try" and "Play from here" take the draft along.
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import { RECORDER_CONFIRM_KEY, confirmOf } from '../flows/confirmDelete';
import { flowsChannel } from '../flows/host';
import { useShell } from '../state/shell';
import type { IpcClient } from '../ipc/client';
import type { FlowRunState, JsonValue, RecordingEntry, RecordingList } from '../ipc/protocol';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import ExportDialog from './ExportDialog.vue';
import { recorderChannel } from './host';
import { withLiveSteps, type LiveRecording } from './live';
import LiveSteps from './LiveSteps.vue';
import { PACE_KEY, PACES, paceOf, type Pace } from './pace';
import { pauseMap } from './pauses';
import {
  FOLLOW_KEY,
  NO_PLAYBACK,
  followOf,
  runningSteps,
  withResult,
  withStep,
  type Playback,
} from './playback';
import { stepValue, type Recording } from './recording';
import RecordingBar from './RecordingBar.vue';
import RecordingNav from './RecordingNav.vue';
import { changedOnDisk, entryOf } from './draftGuard';
import { INSERTED_PAUSE_KEY, insertedPauseOf, pickedOnce } from './editor';
import StepList from './StepList.vue';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    client: IpcClient;
    subscribe?: Subscribe;
    clipboard?: Pick<Clipboard, 'writeText'>;
    /** The export dialog inline instead of in a portal (the tests). */
    inlineDialog?: boolean;
  }>(),
  { subscribe: undefined, clipboard: undefined, inlineDialog: false },
);
const { t } = useI18n();
const channel = recorderChannel(props.client, props.subscribe ?? windowEvents);
const runs = flowsChannel(props.client, props.subscribe ?? windowEvents);
const shell = useShell();

const list = ref<RecordingList | null>(null);
const chosen = ref<string | null>(null);
/** The recording as its file holds it, and the person's edits of it. */
const original = ref<Recording | null>(null);
const draft = ref<Recording | null>(null);
const error = ref<string | null>(null);
const notice = ref<string | null>(null);
const exporting = ref(false);
const flowRun = ref<FlowRunState | null>(null);
const pace = ref<Pace>('fast');
const playback = ref<Playback>(NO_PLAYBACK);
const follow = ref(true);
/** "Delete" asks first (`automation.recorder.confirmDelete`). */
const confirmDelete = ref(true);
/** By step id: the pause before it, as the recording's file holds it. */
const pauses = ref<Map<number, number>>(new Map());
const stepList = ref<InstanceType<typeof StepList> | null>(null);
/** An added step's pause (`automation.recorder.insertedStepPauseMs`, RB-092-01). */
const insertedPause = ref(500);
/** The recording that runs, its steps as they are taken. */
const live = ref<LiveRecording | null>(null);
/** The chosen recording's entry as the panel read it; another recording asked for over edits;
 *  the file changed on disk under unsaved edits; a save going on (RB-092-01 review). */
const known = ref<RecordingEntry | undefined>(undefined);
const pendingChoice = ref<string | null>(null);
const stale = ref(false);
let saving = false;

const dirty = computed(
  () => draft.value !== null && JSON.stringify(draft.value) !== JSON.stringify(original.value),
);
/** The URLs the recording knows, proposed for a step's URL (RB-092-01). */
const urls = computed(() => {
  const known = (draft.value?.steps ?? []).filter((step) => ['navigate', 'waitForUrl'].includes(step.action.type));
  return [...new Set([draft.value?.startUrl ?? '', ...known.map((step) => stepValue(step.action) ?? '')])].filter((url) => url !== '');
});
const runningIds = computed(() => runningSteps(playback.value));
const running = computed(() => flowRun.value?.running ?? null);
const recordingNow = computed(() => live.value?.name ?? list.value?.recordingNow);
const paces = computed(() => PACES.map((value) => ({ value, label: t(`recorder.panel.pace.${value}`) })));
/** "Play"'s tooltip: what it does, and its shortcut from the registry. */
const playHint = computed(() => {
  const shortcut = shell.bindingOf('recorder.play');
  return shortcut === null ? t('recorder.panel.play_hint') : t('recorder.panel.play_shortcut', { shortcut });
});
/** Why "Play" waits, or null. */
const playBlocked = computed(() => {
  if (recordingNow.value !== undefined) {
    return t('recorder.panel.play_recording');
  }
  if (dirty.value) {
    return t('recorder.panel.play_unsaved');
  }
  return running.value === null ? null : t('recorder.panel.play_running');
});

function reason(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}

async function load(): Promise<void> {
  try {
    list.value = await channel.list();
    error.value = list.value.error ?? null;
    const gone =
      !list.value.error && !list.value.recordings.some((entry) => entry.name === chosen.value);
    if (chosen.value !== null && gone) {
      // Deleted (RB-090-91): nothing is left to show or save. A list the core could not read
      // says nothing about it, and unsaved edits stay.
      chosen.value = null;
      original.value = null;
      draft.value = null;
      playback.value = NO_PLAYBACK;
    } else if (!saving && changedOnDisk(known.value, entryOf(list.value, chosen.value))) {
      // Written meanwhile (an agent's step tools, a continued recording): read again, never
      // over unsaved edits - those wait for Revert.
      if (dirty.value) {
        stale.value = true;
      } else if (chosen.value !== null) {
        await choose(chosen.value, true);
      }
    }
  } catch (caught) {
    error.value = t('recorder.panel.error.list', { reason: reason(caught) });
  }
}

/** Opens the recording `name`; over unsaved edits only once the person said so (`force`). */
async function choose(name: string, force = false): Promise<void> {
  if (dirty.value && !force) {
    pendingChoice.value = name;
    return;
  }
  pendingChoice.value = null;
  error.value = null;
  notice.value = null;
  try {
    const { recording, pauses: read } = await channel.get(name);
    chosen.value = name;
    playback.value = NO_PLAYBACK;
    original.value = recording;
    draft.value = recording;
    pauses.value = pauseMap(read);
    known.value = entryOf(list.value, name);
    stale.value = false;
  } catch (caught) {
    error.value = t('recorder.panel.error.open', { name, reason: reason(caught) });
  }
}

function edit(change: (recording: Recording) => Recording): void {
  if (draft.value !== null) {
    draft.value = change(draft.value);
    notice.value = null;
  }
}

async function save(): Promise<void> {
  if (chosen.value === null || draft.value === null || stale.value) {
    return;
  }
  error.value = null;
  saving = true;
  try {
    list.value = await channel.save(chosen.value, draft.value);
    known.value = entryOf(list.value, chosen.value);
    original.value = draft.value;
    notice.value = t('recorder.panel.saved', { name: chosen.value });
    // The saved order has pauses of its own.
    pauses.value = pauseMap((await channel.get(chosen.value)).pauses);
  } catch (caught) {
    error.value = t('recorder.panel.error.save', { reason: reason(caught) });
  } finally {
    saving = false;
  }
}

function revert(): void {
  if (chosen.value !== null) {
    void choose(chosen.value, true);
  }
}

function run(id: string, args?: JsonValue): void {
  error.value = null;
  channel.run(id, args).catch((caught: unknown) => {
    error.value = t('recorder.panel.error.command', { reason: reason(caught) });
  });
}

function play(): void {
  if (chosen.value !== null) {
    playback.value = NO_PLAYBACK;
    run('recorder.play', { recordingId: chosen.value, pace: pace.value });
  }
}

function exported(folder: string): void {
  notice.value = t('recorder.export.saved', { folder });
}

/** An editor command on step `stepId` of the chosen recording (RB-092-01). */
function runStep(id: string, stepId: number, extra: Record<string, JsonValue> = {}): void {
  if (chosen.value !== null) {
    run(id, { recordingId: chosen.value, stepId, ...extra });
  }
}

/** "Try": the step alone in the active view, from the draft when it has unsaved edits. */
function tryStep(stepId: number, draftToTry: Recording | null): void {
  const sent = draftToTry ?? (dirty.value ? draft.value : null);
  runStep('recorder.step_try', stepId, sent === null ? {} : { draft: sent as unknown as JsonValue });
}

/** "Pick in view": the element the next click in the active view picks, or null. */
function pickElement() {
  return pickedOnce(channel, chosen.value, (caught) => {
    error.value = t('recorder.panel.error.command', { reason: reason(caught) });
  });
}

/** Scrolls step `id` into sight when the panel follows the playback. */
function reveal(id: number | null | undefined): void {
  if (!follow.value || id === null || id === undefined) {
    return;
  }
  void nextTick(() => {
    const row = document.querySelector(`[data-testid="recorder-steps"] [data-testid="step-${id}"]`);
    if (row !== null && row !== undefined && typeof row.scrollIntoView === 'function') {
      row.scrollIntoView({ block: 'nearest' });
    }
  });
}

/** The run going on, if any; the panel works without (an older core). */
async function loadRun(): Promise<void> {
  try {
    flowRun.value = await runs.state();
  } catch {
    flowRun.value = null;
  }
  const state = flowRun.value;
  const followed = playback.value.runId;
  if (state?.running !== undefined) {
    if (state.running.flow === chosen.value && state.running.runId !== followed) {
      playback.value = { ...NO_PLAYBACK, runId: state.running.runId };
    }
  } else if (state?.last !== undefined && followed !== null && state.last.runId === followed) {
    // The run followed ended: its result names the step each failed view stopped at.
    const ended = withResult(playback.value, state.last);
    if (JSON.stringify(ended) !== JSON.stringify(playback.value)) {
      playback.value = ended;
      const first = Object.keys(ended.failed)[0];
      reveal(first === undefined ? null : Number(first));
    }
  }
}

/** The choice beside "Play" starts at the setting; following the playback is one too. */
async function loadSettings(): Promise<void> {
  try {
    const answer = await props.client.expect({ type: 'getSettings' }, 'settings');
    const value = (key: string) => answer.values.find((entry) => entry.key === key)?.value;
    pace.value = paceOf(value(PACE_KEY));
    follow.value = followOf(value(FOLLOW_KEY));
    confirmDelete.value = confirmOf(value(RECORDER_CONFIRM_KEY));
    insertedPause.value = insertedPauseOf(value(INSERTED_PAUSE_KEY));
  } catch {
    // The defaults stay: fast, following.
  }
}

const stop = channel.onChanged(() => void load());
const stopRuns = runs.onChanged(() => void loadRun());
// A changed recorder setting applies in the open panel at once (RB-094-23).
const stopSettings = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'settingsChanged' && message.keys.some((key) => key.startsWith('automation.recorder.'))) {
    void loadSettings();
  }
});
const stopSteps = channel.onStep((step) => {
  if (step.flow !== chosen.value) {
    return;
  }
  playback.value = withStep(playback.value, step);
  if (step.state !== 'passed') {
    reveal(step.stepId);
  }
});
// A shortcut's editor command acts on the step in focus (RB-092-01).
const stopCommands = channel.onStepCommand((command) => stepList.value?.runCommand(command));
const stopLive = channel.onLive((steps) => {
  const ended = steps.state === 'ended' && live.value !== null;
  live.value = withLiveSteps(live.value, steps);
  if (ended) {
    void openSaved();
  }
});
/** The recording just saved (the newest), unless the person has unsaved edits. */
async function openSaved(): Promise<void> {
  await load();
  const newest = list.value?.recordings[0];
  if (newest !== undefined && !dirty.value) {
    await choose(newest.name);
  }
}
onMounted(() => {
  shell.loadCommands();
  void load();
  void loadRun();
  void loadSettings();
  // A recording that runs already: its steps so far (an older core answers nothing).
  channel.catchUp().catch(() => undefined);
});
onBeforeUnmount(() => {
  stop();
  stopRuns();
  stopSettings();
  stopSteps();
  stopLive();
  stopCommands();
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-default text-default" data-testid="recorder-page">
    <header class="flex flex-wrap items-center gap-2 border-b border-default px-3 py-2">
      <h2 class="text-sm font-semibold">{{ t('recorder.panel.title') }}</h2>
      <UBadge
        v-if="recordingNow !== undefined"
        color="error"
        variant="soft"
        :label="t('recorder.panel.recording_now', { name: recordingNow })"
        data-testid="recording-now"
      />
      <UButton
        v-if="recordingNow !== undefined"
        size="sm"
        color="error"
        variant="outline"
        :label="t('recorder.panel.stop')"
        data-testid="recorder-stop"
        @click="run('recorder.stop')"
      />
      <UButton
        v-else-if="list?.project === true"
        size="sm"
        color="neutral"
        variant="outline"
        :label="t('recorder.panel.start')"
        data-testid="recorder-start"
        @click="run('recorder.start')"
      />
      <span v-if="list?.folder !== undefined" class="ms-auto truncate text-xs text-muted">{{ list.folder }}</span>
    </header>
    <p v-if="error !== null" class="px-3 py-2 text-sm text-error" role="alert" data-testid="recorder-error">{{ error }}</p>
    <p v-if="list === null && error === null" class="p-3 text-sm text-muted" data-testid="recorder-loading">
      {{ t('recorder.panel.loading') }}
    </p>
    <p v-else-if="list !== null && !list.project" class="p-3 text-sm text-muted" data-testid="recorder-no-project">
      {{ t('recorder.panel.no_project') }}
    </p>
    <div v-else-if="list !== null" class="flex min-h-0 flex-1 flex-col sm:flex-row">
      <RecordingNav
        :list="list"
        :chosen="chosen"
        :confirm="confirmDelete"
        :recording-now="recordingNow"
        :playing="running?.flow ?? null"
        @choose="choose"
        @delete="(name) => run('recorder.delete', { recordingId: name })"
      />
      <section class="flex min-h-0 min-w-0 flex-1 flex-col" :aria-label="t('recorder.panel.steps')">
        <LiveSteps v-if="live !== null" :live="live" />
        <p v-else-if="draft === null" class="p-3 text-sm text-muted" data-testid="recorder-pick">{{ t('recorder.panel.pick') }}</p>
        <template v-else>
          <RecordingBar
            v-model:pace="pace"
            :name="chosen ?? ''"
            :dirty="dirty"
            :notice="notice"
            :running-flow="running?.flow ?? null"
            :paces="paces"
            :play-blocked="playBlocked"
            :play-hint="playHint"
            :stale="stale"
            :pending-choice="pendingChoice"
            @play="play"
            @cancel-run="run('flows.cancel')"
            @revert="revert"
            @save="save"
            @export="exporting = true"
            @discard="pendingChoice !== null && choose(pendingChoice, true)"
            @keep="pendingChoice = null"
          />
          <StepList
            ref="stepList"
            :recording="draft"
            :running-ids="runningIds"
            :failed="playback.failed"
            :pauses="pauses"
            :dirty="dirty"
            :recording-now="recordingNow"
            :inserted-pause="insertedPause"
            :urls="urls"
            :pick-element="pickElement"
            @edit="edit"
            @try="tryStep"
            @play-from="(stepId) => runStep('recorder.play_from', stepId, { pace, ...(dirty && draft !== null ? { draft } : {}) })"
            @continue-from="(stepId) => runStep('recorder.continue_from', stepId)"
          />
        </template>
      </section>
    </div>
    <ExportDialog
      v-if="chosen !== null"
      v-model:open="exporting"
      :channel="channel"
      :name="chosen"
      :clipboard="clipboard"
      :inline="inlineDialog"
      @saved="exported"
    />
  </div>
</template>
