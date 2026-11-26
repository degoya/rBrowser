<script setup lang="ts">
// The findings panel (RB-060-01): the last check run's findings, filtered by severity, plugin and
// view, grouped by rule. A finding not in every checked view says in which breakpoints it is;
// choosing one asks the core to mark its element in those views. Arrow keys move through the
// list. While checks run it shows their progress, one row per plugin, with Cancel, and after the
// run one summary line (`FindingsProgress.vue`, RB-080-98), and each plugin's findings as soon as
// it is through (RB-080-89); a
// rule's Help opens through the core (`ExternalLink.vue`, RB-080-92). A finding that names a width
// (the width sweep's) offers "Show at N px", its plugin's `show` command (RB-093-01). The logic is `findings.ts`
// and `progress.ts`; the IPC is a `FindingsChannel` (`types.ts`). Every control is Nuxt UI
// (RB-080-88): the filters are `FindingsFilters.vue`, each rule a `UAccordion` item, open until
// the reader folds it, and what the project turned off a `UCollapsible` (`ProjectOffNote.vue`). Export writes the run as a
// file (`FindingsExport.vue`, RB-092-05). One check or a selection runs again from the progress
// details or the plugin filter over the last run's target and page; the core merges the result
// into the last run (RB-094-17). RB-099-10: Run checks and visual regression (`VisualStart.vue`)
// each run over the views of their scope pick list (`ScopePicker.vue`), the last choice kept per
// function; a run again takes the scope of the run before, and the run names the views checked.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAccordion from '@nuxt/ui/components/Accordion.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import ExternalLink from '../links/ExternalLink.vue';
import ScopePicker from '../scope/ScopePicker.vue';
import { useScope } from '../scope/useScope';
import { scopeOf, withScope } from '../../../plugins/web-common/panel-ui/src/scope';
import FindingsExport from './FindingsExport.vue';
import FindingsFilters from './FindingsFilters.vue';
import FindingsProgress from './FindingsProgress.vue';
import ProjectOffNote from './ProjectOffNote.vue';
import VisualStart from './VisualStart.vue';
import { EXPORT_COMMAND, exportRequest, type ExportFormat } from './exporting';
import { FINDINGS_ICONS } from './icons';
import {
  SEVERITIES,
  countBySeverity,
  failureParams,
  filterFindings,
  groupByRule,
  highlightRequest,
  keepChoice,
  moveFocus,
  nestMessages,
  onlyIn,
  pluginsOf,
  rerunScope,
  sentence,
  showWidthRequest,
} from './findings';
import type { BreakpointSummary, JsonValue, ProjectOff } from '../ipc/protocol';
import type {
  CheckFailure,
  CheckedView,
  ChecksProgress,
  FindingsChannel,
  FindingsEvent,
  FindingsRequest,
  FindingsRun,
  MergedFinding,
  ProgressEvent,
  Severity,
} from './types';

const props = withDefaults(defineProps<{ channel: FindingsChannel; breakpoints?: BreakpointSummary[] }>(), { breakpoints: () => [] });
const { t, mergeLocaleMessage } = useI18n();

const run = ref<FindingsRun | null>(null);
const projectOff = ref<ProjectOff | null>(null);
/** Asked for and not yet told it is over. */
const running = ref(false);
/** The progress of the run in progress or the last one, with when it came. */
const progress = ref<ChecksProgress | null>(null);
const progressAt = ref(0);
const now = ref(Date.now());
const failed = ref(false);
const selected = ref<string | null>(null);
const severities = ref<Severity[]>([...SEVERITIES]);
const plugin = ref('');
const view = ref<number | ''>('');
const list = ref<HTMLElement | null>(null);
const scope = useScope('checks', () => props.breakpoints.map((breakpoint) => breakpoint.id));

function show(event: FindingsEvent): void {
  mergeLocaleMessage('en', nestMessages(event.messages));
  run.value = event.run;
  projectOff.value = event.projectOff ?? null;
  if (event.partial !== true) {
    running.value = false;
    selected.value = null;
  }
}

function track(event: ProgressEvent): void {
  const { type: _type, ...next } = event;
  progress.value = next;
  progressAt.value = Date.now();
  now.value = progressAt.value;
  running.value = !next.finished;
}

const stop = props.channel.on((event) => {
  if (event.type === 'findings') {
    show(event);
  } else {
    track(event);
  }
});
onBeforeUnmount(stop);
// A run that ended before the panel opened; one that arrived meanwhile is newer.
void props.channel.last().then(
  (event) => {
    if (event !== null && run.value === null) {
      show(event);
    }
  },
  () => undefined,
);
// A run still going when the panel opened.
void props.channel.progress().then(
  (event) => {
    if (event !== null && progress.value === null) {
      track(event);
    }
  },
  () => undefined,
);

/** The run in progress, as the progress says. */
const going = computed(() => progress.value !== null && !progress.value.finished);
/** The time on the bar: the core's as of its message, ticking on between messages. */
const elapsedMs = computed(() => (progress.value === null ? 0 : progress.value.elapsedMs + (now.value - progressAt.value)));
let ticker: ReturnType<typeof setInterval> | null = null;
watch(
  going,
  (on) => {
    if (on && ticker === null) {
      ticker = setInterval(() => (now.value = Date.now()), 1000);
    } else if (!on && ticker !== null) {
      clearInterval(ticker);
      ticker = null;
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  if (ticker !== null) {
    clearInterval(ticker);
  }
});

const shown = computed(() =>
  run.value === null
    ? []
    : filterFindings(run.value.findings, {
        severities: severities.value,
        plugin: plugin.value,
        view: view.value === '' ? null : view.value,
      }),
);
const groups = computed(() => groupByRule(shown.value));
const ruleItems = computed(() => groups.value.map((group) => ({ value: group.ruleId, group })));
/** The rules open: a rule opens when it first shows, and stays as the reader leaves it. */
const rulesOpen = ref<string[]>([]);
const seenRules = new Set<string>();
watch(
  groups,
  (now) => {
    const fresh = now.map((group) => group.ruleId).filter((id) => !seenRules.has(id));
    fresh.forEach((id) => seenRules.add(id));
    if (fresh.length > 0) {
      rulesOpen.value = [...rulesOpen.value, ...fresh];
    }
  },
  { immediate: true },
);
const counts = computed(() => countBySeverity(run.value?.findings ?? []));
const plugins = computed(() => (run.value === null ? [] : pluginsOf(run.value)));
const viewChoices = computed(() => (run.value?.views ?? []).map((checked) => ({ id: checked.id, name: viewName(checked) })));
// A choice the run no longer has (a plugin's own run replaced it) falls back to all (RB-094-14).
watch([plugins, viewChoices], ([ids, views]) => {
  plugin.value = keepChoice(plugin.value, ids);
  view.value = keepChoice(view.value, views.map((checked) => checked.id));
});

function viewName(checked: CheckedView): string {
  return checked.breakpoint ?? t('findings.view_id', { id: String(checked.id) });
}

function onlyInText(finding: MergedFinding): string | null {
  const views = run.value === null ? null : onlyIn(finding, run.value);
  return views === null ? null : t('findings.only_in', { breakpoints: views.map(viewName).join(', ') });
}

/** The views the run checked, by breakpoint: views not named here were not checked. */
const checked = computed(() => ((run.value?.views.length ?? 0) === 0 ? null : t('findings.checked', { views: run.value?.views.map(viewName).join(', ') ?? '' })));

function message(finding: MergedFinding): string {
  return t(finding.message.key, finding.message.params ?? {});
}

async function send(request: Parameters<FindingsChannel['request']>[0]): Promise<boolean> {
  try {
    await props.channel.request(request);
    return true;
  } catch {
    failed.value = true;
    return false;
  }
}

// Run disables itself while checks run: the focus it had goes to Cancel, the one action that ends
// the run, and back to Run after it - unless the reader moved it meanwhile (RB-090-23).
const runButton = ref<{ $el: HTMLElement } | null>(null);
const section = ref<HTMLElement | null>(null);
let handOff = false;

/** Whether the focus is nowhere a reader put it: the body, or a control that went or disabled. */
function focusLost(): boolean {
  const active = document.activeElement;
  const cancel = section.value?.querySelector('[data-testid="cancel"]');
  const disabled = active instanceof HTMLButtonElement && active.disabled;
  return active === null || active === document.body || disabled || active === runButton.value?.$el || (active === cancel && cancel !== null);
}

watch([running, () => progress.value !== null && !progress.value.finished], async ([on, cancellable]) => {
  if (!handOff) {
    return;
  }
  await nextTick();
  if (!focusLost()) {
    handOff = false;
  } else if (on && cancellable) {
    section.value?.querySelector<HTMLElement>('[data-testid="cancel"]')?.focus();
  } else if (!on) {
    handOff = false;
    runButton.value?.$el.focus();
  }
});

async function runChecks(): Promise<void> {
  handOff = document.activeElement === runButton.value?.$el;
  failed.value = false;
  running.value = true;
  progress.value = null;
  const request = withScope<Extract<FindingsRequest, { type: 'runChecks' }>>({ type: 'runChecks', target: 'all' }, scope.value);
  if (!(await send(request))) {
    running.value = false;
  }
}

/** The breakpoints of the run shown, when it covered fewer than the project's (RB-099-10). */
function rerunViews(last: FindingsRun | null): string[] | null {
  return last === null ? null : rerunScope(last, props.breakpoints.map((breakpoint) => breakpoint.id));
}

function runVisual(id: string, args: Record<string, JsonValue> | null): void {
  void send(args === null ? { type: 'runCommand', id } : { type: 'runCommand', id, args });
}

/** Runs `plugins` again over the last run's target and page, merged into it by the core
 * (RB-094-17); the focus goes to Cancel and back like Run's. */
async function rerunChecks(plugins: string[]): Promise<void> {
  if (plugins.length === 0 || running.value) {
    return;
  }
  handOff = true;
  failed.value = false;
  running.value = true;
  progress.value = null;
  const last = run.value;
  const views = rerunViews(last);
  const request: FindingsRequest = {
    type: 'runChecks',
    target: last?.target ?? 'all',
    plugins,
    ...(last?.url === undefined ? {} : { url: last.url }),
    ...(views === null ? {} : { scope: views }),
  };
  if (!(await send(request))) {
    running.value = false;
  }
}

function cancelChecks(): void {
  void send({ type: 'runCommand', id: 'checks.cancel' });
}

/** The last run as a file (RB-092-05): the setting's format, or the one chosen. */
function exportRun(format: ExportFormat | null): void {
  void send(exportRequest(format));
}

/** A plugin's failure as one sentence: the view, and the reason's text when there is a code. */
function failureText(failure: CheckFailure): string {
  if (failure.code === undefined) {
    return t('findings.plugin_failed', { plugin: failure.plugin });
  }
  const checked = run.value?.views.find((view) => view.id === failure.view);
  const where = checked === undefined ? '' : t('findings.plugin_failed_view', { view: viewName(checked) });
  // The code's parameters, the host's names of the plugin last, a plugin's own reason only; one
  // sentence, its end punctuated, before the panel's closing one (RB-099-06).
  const reason = sentence(t(failure.code, failureParams(failure.code, failure.plugin, failure.params)));
  return t('findings.plugin_failed_reason', { plugin: failure.plugin, where, reason });
}

function choose(finding: MergedFinding): void {
  selected.value = finding.id;
  const request = highlightRequest(finding);
  if (request !== null) {
    void send(request);
  }
}

/** "Show at N px" (RB-093-01): the element marked, then the view held at the finding's width. */
function showWidth(finding: MergedFinding): void {
  const show = showWidthRequest(finding);
  if (show !== null) {
    choose(finding);
    void send(show.request);
  }
}

function openRules(): void {
  void send({ type: 'openSettings', key: 'checks.rules' });
}

/** The arrow keys move through the findings of every open rule; the rules' own triggers keep
 * the accordion's arrow keys. */
async function step(event: KeyboardEvent, direction: 1 | -1): Promise<void> {
  const buttons = [...(list.value?.querySelectorAll<HTMLButtonElement>('[data-finding]') ?? [])];
  const current = buttons.findIndex((button) => button === event.target);
  const next = moveFocus(current, buttons.length, direction);
  await nextTick();
  buttons[next]?.focus();
}

const severityClass: Record<Severity, string> = {
  error: 'text-error',
  warning: 'text-warning',
  info: 'text-info',
};
</script>

<template>
  <section
    ref="section"
    class="flex flex-col gap-3 p-3 text-default"
    aria-labelledby="findings-title"
    data-testid="findings-panel"
  >
    <header class="flex flex-wrap items-center gap-3">
      <h2 id="findings-title" class="text-base font-semibold">{{ t('findings.title') }}</h2>
      <UButton
        ref="runButton"
        size="sm"
        :loading="running"
        :disabled="running || scopeOf(scope) === null"
        data-testid="run"
        @click="runChecks"
      >
        {{ t('findings.run') }}
      </UButton>
      <UButton size="sm" variant="ghost" data-testid="rules" @click="openRules">
        {{ t('findings.rules_settings') }}
      </UButton>
      <FindingsExport
        v-if="run"
        :disabled="running"
        :shortcut="channel.binding?.(EXPORT_COMMAND) ?? null"
        @export="exportRun"
      />
      <p v-if="run && !running" class="text-sm text-muted" data-testid="summary">
        {{ t('findings.summary_count', { total: run.summary.total, plugins: run.plugins.length }) }}
      </p>
      <ScopePicker v-model="scope" :breakpoints="breakpoints" :disabled="running" class="w-full" />
      <p v-if="run && !running && checked" class="w-full text-xs text-muted" data-testid="checked">{{ checked }}</p>
    </header>

    <ProjectOffNote :project-off="projectOff" />
    <VisualStart :breakpoints="breakpoints" :busy="running" @run="runVisual" />

    <FindingsProgress
      v-if="progress"
      :progress="progress"
      :elapsed-ms="elapsedMs"
      :can-rerun="run !== null && !running"
      @cancel="cancelChecks"
      @rerun="rerunChecks"
    />
    <p v-else-if="running" role="status" class="text-sm">{{ t('findings.running') }}</p>
    <p v-if="!running && progress?.cancelled" class="text-sm text-muted" data-testid="cancelled">
      {{ t('findings.cancelled') }}
    </p>
    <p v-if="failed" role="alert" class="text-sm text-error">{{ t('findings.failed') }}</p>
    <p v-if="!run && !running" class="text-sm text-muted" data-testid="no-run">{{ t('findings.no_run') }}</p>

    <template v-if="run">
      <FindingsFilters
        v-model:severities="severities"
        v-model:plugin="plugin"
        v-model:view="view"
        :counts="counts"
        :plugins="plugins"
        :views="viewChoices"
        :busy="running"
        @rerun="(id) => rerunChecks([id])"
      />

      <p
        v-for="failure in run.failures ?? []"
        :key="`${failure.plugin}-${failure.view ?? ''}`"
        class="text-sm text-error"
        data-testid="failure"
      >
        {{ failureText(failure) }}
      </p>
      <p v-if="shown.length === 0" class="text-sm text-muted" data-testid="empty">{{ t('findings.none') }}</p>

      <div ref="list">
        <UAccordion
          v-model="rulesOpen"
          type="multiple"
          :items="ruleItems"
          :ui="{ trigger: 'gap-2 py-1.5 text-sm', label: 'min-w-0 flex-1', trailingIcon: 'size-4', body: 'pb-2 pt-0 text-sm' }"
        >
          <template #default="{ item }">
            <span class="flex items-center gap-2 font-semibold" :data-testid="`rule-${item.group.ruleId}`">
              <span :class="severityClass[item.group.severity]">{{ t(`findings.severity.${item.group.severity}`) }}</span>
              <code>{{ item.group.ruleId }}</code>
              <span class="font-normal text-muted">{{ t('findings.count', { count: String(item.group.findings.length) }) }}</span>
            </span>
          </template>
          <template #body="{ item }">
            <ul class="flex flex-col gap-1">
              <li v-for="finding in item.group.findings" :key="finding.id" class="flex items-start gap-2">
                <UButton
                  data-finding
                  :data-testid="`finding-${finding.id}`"
                  :aria-pressed="selected === finding.id"
                  size="sm"
                  color="neutral"
                  :variant="selected === finding.id ? 'soft' : 'ghost'"
                  class="flex-1 flex-col items-start gap-0 text-left font-normal"
                  @click="choose(finding)"
                  @keydown.down.prevent="step($event, 1)"
                  @keydown.up.prevent="step($event, -1)"
                >
                  <span>{{ message(finding) }}</span>
                  <span class="text-xs text-muted">
                    <code v-if="finding.element?.selector ?? finding.element?.path">{{ finding.element?.selector ?? finding.element?.path }}</code>
                    {{ finding.plugins.join(', ') }}
                  </span>
                  <span v-if="onlyInText(finding)" class="text-xs" data-testid="only-in">{{ onlyInText(finding) }}</span>
                </UButton>
                <!-- A secondary action: an icon, named and explained by its tooltip. -->
                <UButton
                  v-if="showWidthRequest(finding)"
                  :data-testid="`show-width-${finding.id}`"
                  size="sm"
                  color="neutral"
                  variant="ghost"
                  class="shrink-0"
                  :icon="FINDINGS_ICONS.showWidth"
                  :aria-label="t('findings.show_width', { width: showWidthRequest(finding)?.width ?? 0 })"
                  :title="t('findings.show_width_hint', { width: showWidthRequest(finding)?.width ?? 0 })"
                  @click="showWidth(finding)"
                />
                <ExternalLink
                  v-if="finding.helpUrl"
                  :href="finding.helpUrl"
                  :data-testid="`help-${finding.id}`"
                  class="shrink-0 py-1.5 text-xs underline focus-visible:outline-2 focus-visible:outline-primary"
                >{{ t('findings.help') }}</ExternalLink>
              </li>
            </ul>
          </template>
        </UAccordion>
      </div>
    </template>
  </section>
</template>
