<script setup lang="ts">
// The Flows panel (RB-090-60, RB-040-07's panel), a core panel of the panels page
// (`panels/core.ts`, `panels.flows`, Ctrl+Shift+Space F F): the project's flows
// (`.rbrowser/flows/<name>.ts`, `listFlows`) with "Run" in all views or in the view chosen above
// the list, "Run all", the run going on with each view's status (waiting, running, passed,
// failed, skipped), how long it runs and the flows of "Run all" still waiting - asked again every
// second while it runs - and "Stop"; while it runs the Run buttons wait and say why (RB-090-71).
// Then the last result: passed or failed per view, where a failed
// view broke (its step and line, the message) and its screenshot's file, with "Open folder"
// (`flows.open_results`). Every action is a command of the registry (`flows.*`), as the keys,
// the palette and the page menu run them; the trust gate stays the core's.
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import type { IpcClient } from '../ipc/client';
import type { FlowRunState, FlowSummary, JsonValue } from '../ipc/protocol';
import { elapsedSeconds } from '../activity/activities';
import { clock } from '../portal/portal';
import { RECORDER_ICONS } from '../recorder/icons';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import { failurePlace, statusLook, viewLabel } from './flows';
import { flowsChannel } from './host';

defineOptions({ inheritAttrs: false });
const props = withDefaults(
  defineProps<{
    client: IpcClient;
    subscribe?: Subscribe;
    /** How often a run going on is asked for, in ms. */
    pollMs?: number;
    /** The clock of the elapsed time, ms since the epoch. */
    now?: () => number;
  }>(),
  { subscribe: undefined, pollMs: 1000, now: () => Date.now() },
);
const { t, te } = useI18n();
const shell = useShell();
const channel = flowsChannel(props.client, props.subscribe ?? windowEvents);
const when = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });

const flows = ref<FlowSummary[] | null>(null);
const state = ref<FlowRunState | null>(null);
const error = ref<string | null>(null);
/** Where "Run" runs: `all`, or a view's id. */
const target = ref('all');

const views = computed(() => shell.state.value?.views ?? []);
const targets = computed(() => [
  { value: 'all', label: t('flows.panel.all_views') },
  ...views.value.map((view) => ({ value: String(view.id), label: view.label })),
]);
const targetName = computed(
  () => targets.value.find((item) => item.value === target.value)?.label ?? t('flows.panel.all_views'),
);
const running = computed(() => state.value?.running ?? null);
const last = computed(() => state.value?.last ?? null);
/** The clock of the elapsed time; ticks with each poll while a run goes on. */
const nowMs = ref(props.now());
const elapsed = computed(() =>
  running.value === null ? '' : clock(elapsedSeconds(running.value.startedAtMs, nowMs.value)),
);
/** Why Run and Run all wait: the run going on. */
const busy = computed(() => (running.value === null ? null : t('flows.panel.busy', { flow: running.value.flow })));

function statusText(status: string): string {
  const key = `flows.panel.status.${status}`;
  return te(key) ? t(`flows.panel.status.${status}`) : status;
}

function reason(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught);
}

async function load(): Promise<void> {
  nowMs.value = props.now();
  try {
    const [list, now] = await Promise.all([channel.flows(), channel.state()]);
    flows.value = list;
    state.value = now;
  } catch (caught) {
    error.value = t('flows.panel.error', { reason: reason(caught) });
  }
}

async function refresh(): Promise<void> {
  nowMs.value = props.now();
  try {
    state.value = await channel.state();
  } catch (caught) {
    error.value = t('flows.panel.error', { reason: reason(caught) });
  }
}

function run(id: string, args?: JsonValue): void {
  error.value = null;
  channel.run(id, args).catch((caught: unknown) => {
    error.value = t('flows.panel.error', { reason: reason(caught) });
  });
}

function runFlow(name: string): void {
  const view = Number(target.value);
  run('flows.run', target.value === 'all' || Number.isNaN(view) ? { name } : { name, views: [view] });
}

let timer: ReturnType<typeof setInterval> | undefined;
watch(running, (now) => {
  if (now !== null && timer === undefined) {
    timer = setInterval(() => void refresh(), props.pollMs);
  } else if (now === null && timer !== undefined) {
    clearInterval(timer);
    timer = undefined;
  }
});
const stop = channel.onChanged(() => void load());
onMounted(() => void load());
onBeforeUnmount(() => {
  stop();
  clearInterval(timer);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col overflow-auto bg-default text-default" data-testid="flows-page">
    <header class="flex flex-wrap items-center gap-2 border-b border-default px-3 py-2">
      <h2 class="text-sm font-semibold">{{ t('flows.panel.title') }}</h2>
      <span class="ms-auto flex flex-wrap gap-1">
        <UButton
          v-if="running !== null"
          :icon="RECORDER_ICONS.flowCancel"
          size="sm"
          color="error"
          variant="solid"
          :label="t('flows.panel.stop')"
          data-testid="flows-stop"
          @click="run('flows.cancel')"
        />
        <UButton
          :icon="RECORDER_ICONS.flowRun"
          size="sm"
          :label="t('flows.panel.run_all')"
          :title="busy ?? undefined"
          :disabled="(flows?.length ?? 0) === 0 || running !== null"
          data-testid="flows-run-all"
          @click="run('flows.run_all')"
        />
      </span>
    </header>
    <p v-if="error !== null" class="px-3 py-2 text-sm text-error" role="alert" data-testid="flows-error">{{ error }}</p>
    <p v-if="busy !== null" class="px-3 py-2 text-xs text-muted" data-testid="flows-busy">{{ busy }}</p>
    <p v-if="state === null && error === null" class="p-3 text-sm text-muted" data-testid="flows-loading">
      {{ t('flows.panel.loading') }}
    </p>
    <p v-else-if="state !== null && !state.project" class="p-3 text-sm text-muted" data-testid="flows-no-project">
      {{ t('flows.panel.no_project') }}
    </p>
    <template v-else-if="state !== null">
      <URadioGroup
        v-if="views.length > 0"
        v-model="target"
        :items="targets"
        :legend="t('flows.panel.target')"
        orientation="horizontal"
        size="sm"
        class="border-b border-default px-3 py-2"
        data-testid="flow-target"
      />
      <p v-if="flows !== null && flows.length === 0" class="p-3 text-sm text-muted" data-testid="flows-empty">
        {{ t('flows.panel.empty') }}
      </p>
      <ul :aria-label="t('flows.panel.list')">
        <li
          v-for="flow in flows ?? []"
          :key="flow.name"
          class="flex items-center gap-2 border-b border-default px-3 py-2"
          :data-testid="`flow-${flow.name}`"
        >
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-sm font-medium">{{ flow.name }}</span>
            <span v-if="flow.description" class="truncate text-xs text-muted">{{ flow.description }}</span>
          </span>
          <UButton
            :icon="RECORDER_ICONS.flowRun"
            size="sm"
            color="neutral"
            variant="outline"
            :label="t('flows.panel.run')"
            :title="busy ?? t('flows.panel.run_flow', { flow: flow.name, target: targetName })"
            :disabled="running !== null"
            data-testid="flow-run"
            @click="runFlow(flow.name)"
          />
        </li>
      </ul>
      <section v-if="running !== null" class="border-b border-default px-3 py-2" aria-live="polite" data-testid="flows-running">
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="text-sm font-medium">{{ t('flows.panel.running', { flow: running.flow }) }}</h3>
          <!-- The time ticks every second: not announced each time. -->
          <span class="text-xs text-muted tabular-nums" aria-live="off" data-testid="flows-elapsed">
            {{ t('flows.panel.elapsed', { time: elapsed }) }}
          </span>
        </div>
        <ul class="mt-1 flex flex-wrap gap-2">
          <li v-for="view in running.views" :key="view.view" :data-testid="`running-view-${view.view}`">
            <UBadge
              :color="statusLook(view.status).color"
              :icon="statusLook(view.status).icon"
              variant="soft"
              :label="`${viewLabel(views, view.view)}: ${statusText(view.status)}`"
            />
          </li>
        </ul>
        <p v-if="(running.queued?.length ?? 0) > 0" class="mt-1 text-xs text-muted" data-testid="flows-queued">
          {{ t('flows.panel.queued', { flows: running.queued?.join(', ') }) }}
        </p>
      </section>
      <section v-if="last !== null" class="px-3 py-2" data-testid="flows-result">
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="text-sm font-medium">{{ t('flows.panel.result', { flow: last.flow }) }}</h3>
          <UBadge
            :color="last.passed ? 'success' : 'error'"
            variant="solid"
            :label="statusText(last.passed ? 'passed' : 'failed')"
            data-testid="flows-result-status"
          />
          <span class="text-xs text-muted">
            {{ t('flows.panel.result_meta', { when: when.format(new Date(last.startedAt)), seconds: (last.durationMs / 1000).toFixed(1) }) }}
          </span>
          <UButton
            :icon="RECORDER_ICONS.flowReveal"
            size="sm"
            color="neutral"
            variant="ghost"
            class="ms-auto"
            :label="t('flows.panel.open_results')"
            data-testid="flows-open-results"
            @click="run('flows.open_results')"
          />
        </div>
        <p v-if="last.error" class="mt-1 text-sm text-error">{{ last.error }}</p>
        <ul class="mt-2 flex flex-col gap-2">
          <li
            v-for="view in last.views"
            :key="view.viewId"
            class="rounded-md border border-default p-2"
            :data-testid="`result-view-${view.viewId}`"
          >
            <div class="flex flex-wrap items-center gap-2">
              <UBadge
                :color="statusLook(view.status).color"
                :icon="statusLook(view.status).icon"
                variant="soft"
                :label="statusText(view.status)"
              />
              <span class="text-sm font-medium">{{ view.label }}</span>
              <span class="text-xs text-muted">{{ t('flows.panel.seconds', { seconds: (view.durationMs / 1000).toFixed(1) }) }}</span>
            </div>
            <template v-if="view.error">
              <p v-if="failurePlace(view).step !== null" class="mt-1 text-xs">
                {{
                  failurePlace(view).line !== null
                    ? t('flows.panel.failed_at', { step: failurePlace(view).step, line: failurePlace(view).line })
                    : t('flows.panel.failed_step', { step: failurePlace(view).step })
                }}
              </p>
              <pre class="mt-1 overflow-auto text-xs whitespace-pre-wrap text-error">{{ view.error.message }}</pre>
            </template>
            <p v-if="view.screenshot" class="mt-1 text-xs text-muted">
              {{ t('flows.panel.screenshot', { file: view.screenshot }) }}
            </p>
            <p v-if="view.reason" class="mt-1 text-xs text-muted">{{ t('flows.panel.skipped_why', { reason: view.reason }) }}</p>
          </li>
        </ul>
      </section>
    </template>
  </div>
</template>
