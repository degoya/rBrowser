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
// (`recorder.start`); the core's `recordingsChanged` (a recording stopped or saved) refreshes the
// list, and a recording with unsaved edits is never replaced under the person's hands. While the
// chosen recording plays (RB-090-84) its steps follow the run (`playback.ts`): the step a view
// runs is marked, a failed one red with its reason, and with `automation.recorder.followPlayback`
// the running step scrolls into sight; each step shows the pause Play waits before it at the pace
// As recorded - the exporter's, answered with the recording, left out while edits are unsaved.
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import { flowsChannel } from '../flows/host';
import { useShell } from '../state/shell';
import type { IpcClient } from '../ipc/client';
import type { FlowRunState, JsonValue, RecordingList } from '../ipc/protocol';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import ExportDialog from './ExportDialog.vue';
import { recorderChannel } from './host';
import { RECORDER_ICONS } from './icons';
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
import { deleteStep, moveStep, renameStep, setStepValue, type Recording } from './recording';
import StepRow from './StepRow.vue';

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
const when = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
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
/** By step id: the pause before it, as the recording's file holds it. */
const pauses = ref<Map<number, number>>(new Map());
const stepList = ref<HTMLElement | null>(null);

const dirty = computed(
  () => draft.value !== null && JSON.stringify(draft.value) !== JSON.stringify(original.value),
);
const steps = computed(() => draft.value?.steps ?? []);
const runningIds = computed(() => runningSteps(playback.value));
const running = computed(() => flowRun.value?.running ?? null);
const paces = computed(() => PACES.map((value) => ({ value, label: t(`recorder.panel.pace.${value}`) })));
/** "Play"'s tooltip: what it does, and its shortcut from the registry. */
const playHint = computed(() => {
  const shortcut = shell.bindingOf('recorder.play');
  return shortcut === null ? t('recorder.panel.play_hint') : t('recorder.panel.play_shortcut', { shortcut });
});
/** Why "Play" waits, or null. */
const playBlocked = computed(() => {
  if (list.value?.recordingNow !== undefined) {
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
  } catch (caught) {
    error.value = t('recorder.panel.error.list', { reason: reason(caught) });
  }
}

async function choose(name: string): Promise<void> {
  error.value = null;
  notice.value = null;
  try {
    const { recording, pauses: known } = await channel.get(name);
    chosen.value = name;
    playback.value = NO_PLAYBACK;
    original.value = recording;
    draft.value = recording;
    pauses.value = pauseMap(known);
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
  if (chosen.value === null || draft.value === null) {
    return;
  }
  error.value = null;
  try {
    list.value = await channel.save(chosen.value, draft.value);
    original.value = draft.value;
    notice.value = t('recorder.panel.saved', { name: chosen.value });
    // The saved order has pauses of its own.
    pauses.value = pauseMap((await channel.get(chosen.value)).pauses);
  } catch (caught) {
    error.value = t('recorder.panel.error.save', { reason: reason(caught) });
  }
}

function revert(): void {
  if (chosen.value !== null) {
    void choose(chosen.value);
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

/** Scrolls step `id` into sight when the panel follows the playback. */
function reveal(id: number | null | undefined): void {
  if (!follow.value || id === null || id === undefined) {
    return;
  }
  void nextTick(() => {
    const row = stepList.value?.querySelector(`[data-testid="step-${id}"]`);
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
  } catch {
    // The defaults stay: fast, following.
  }
}

const stop = channel.onChanged(() => void load());
const stopRuns = runs.onChanged(() => void loadRun());
const stopSteps = channel.onStep((step) => {
  if (step.flow !== chosen.value) {
    return;
  }
  playback.value = withStep(playback.value, step);
  if (step.state !== 'passed') {
    reveal(step.stepId);
  }
});
onMounted(() => {
  shell.loadCommands();
  void load();
  void loadRun();
  void loadSettings();
});
onBeforeUnmount(() => {
  stop();
  stopRuns();
  stopSteps();
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-default text-default" data-testid="recorder-page">
    <header class="flex flex-wrap items-center gap-2 border-b border-default px-3 py-2">
      <h2 class="text-sm font-semibold">{{ t('recorder.panel.title') }}</h2>
      <UBadge
        v-if="list?.recordingNow !== undefined"
        color="error"
        variant="soft"
        :label="t('recorder.panel.recording_now', { name: list.recordingNow })"
        data-testid="recording-now"
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
      <nav
        class="max-h-48 shrink-0 overflow-auto border-b border-default sm:max-h-none sm:w-56 sm:border-e sm:border-b-0"
        :aria-label="t('recorder.panel.list')"
      >
        <p v-if="list !== null && list.recordings.length === 0" class="p-3 text-sm text-muted" data-testid="recorder-empty">
          {{ t('recorder.panel.empty') }}
        </p>
        <ul>
          <li v-for="entry in list?.recordings ?? []" :key="entry.name">
            <UButton
              block
              color="neutral"
              :variant="entry.name === chosen ? 'soft' : 'ghost'"
              class="justify-start rounded-none text-start"
              :aria-current="entry.name === chosen ? 'true' : undefined"
              :data-testid="`recording-${entry.name}`"
              @click="choose(entry.name)"
            >
              <span class="flex min-w-0 flex-col">
                <span class="truncate font-medium">{{ entry.name }}</span>
                <span class="text-xs text-muted">
                  {{ t('recorder.panel.entry', { steps: entry.steps, when: when.format(entry.modifiedMs) }) }}
                </span>
              </span>
            </UButton>
          </li>
        </ul>
      </nav>
      <section class="flex min-h-0 min-w-0 flex-1 flex-col" :aria-label="t('recorder.panel.steps')">
        <p v-if="draft === null" class="p-3 text-sm text-muted" data-testid="recorder-pick">{{ t('recorder.panel.pick') }}</p>
        <template v-else>
          <div class="flex flex-wrap items-center gap-2 border-b border-default px-3 py-2">
            <span class="truncate text-sm font-medium">{{ chosen }}</span>
            <span v-if="dirty" class="text-xs text-warning" data-testid="recorder-dirty">{{ t('recorder.panel.unsaved') }}</span>
            <span v-if="notice !== null" class="text-xs text-success" role="status" data-testid="recorder-notice">{{ notice }}</span>
            <span class="text-xs text-muted" role="status" aria-live="polite" data-testid="recorder-running">{{
              running === null ? '' : t('recorder.panel.running', { flow: running.flow })
            }}</span>
            <span class="ms-auto flex flex-wrap items-center gap-1">
              <USelect
                v-model="pace"
                :items="paces"
                size="sm"
                class="w-40"
                :aria-label="t('recorder.panel.pace.label')"
                data-testid="recorder-pace"
              />
              <UButton
                v-if="running !== null"
                :icon="RECORDER_ICONS.flowCancel"
                size="sm"
                color="error"
                variant="soft"
                :label="t('recorder.panel.cancel')"
                data-testid="recorder-cancel"
                @click="run('flows.cancel')"
              />
              <UButton
                :icon="RECORDER_ICONS.flowRun"
                size="sm"
                :label="t('recorder.panel.play')"
                :disabled="playBlocked !== null"
                :title="playBlocked ?? playHint"
                data-testid="recorder-play"
                @click="play"
              />
              <UButton
                :icon="RECORDER_ICONS.recorderRevert"
                size="sm"
                color="neutral"
                variant="ghost"
                :label="t('recorder.panel.revert')"
                :disabled="!dirty"
                data-testid="recorder-revert"
                @click="revert"
              />
              <UButton
                :icon="RECORDER_ICONS.recorderSave"
                size="sm"
                :label="t('recorder.panel.save')"
                :disabled="!dirty"
                data-testid="recorder-save"
                @click="save"
              />
              <UButton
                :icon="RECORDER_ICONS.recorderExport"
                size="sm"
                color="neutral"
                variant="outline"
                :label="t('recorder.panel.export')"
                :disabled="dirty"
                :title="dirty ? t('recorder.panel.export_unsaved') : undefined"
                data-testid="recorder-export"
                @click="exporting = true"
              />
            </span>
          </div>
          <p v-if="steps.length === 0" class="p-3 text-sm text-muted">{{ t('recorder.panel.no_steps') }}</p>
          <ol ref="stepList" class="min-h-0 flex-1 overflow-auto" data-testid="recorder-steps">
            <StepRow
              v-for="(step, index) in steps"
              :key="step.id"
              :step="step"
              :index="index"
              :count="steps.length"
              :running="runningIds.has(step.id)"
              :failure="playback.failed[step.id] ?? null"
              :pause-ms="dirty ? 0 : (pauses.get(step.id) ?? 0)"
              @rename="(name) => edit((recording) => renameStep(recording, step.id, name))"
              @value="(value) => edit((recording) => setStepValue(recording, step.id, value))"
              @move="(by) => edit((recording) => moveStep(recording, step.id, by))"
              @remove="edit((recording) => deleteStep(recording, step.id))"
            />
          </ol>
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
