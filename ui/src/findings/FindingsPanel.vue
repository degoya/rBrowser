<script setup lang="ts">
// The findings panel (RB-060-01): the last check run's findings, filtered by severity, plugin and
// view, grouped by rule. A finding not in every checked view says in which breakpoints it is;
// choosing one asks the core to mark its element in those views. Arrow keys move through the
// list. While checks run it shows their progress, one row per plugin, with Cancel, and after the
// run one summary line (`FindingsProgress.vue`, RB-080-98), and each plugin's findings as soon as
// it is through (RB-080-89); a
// rule's Help opens through the core (`ExternalLink.vue`, RB-080-92). The logic is `findings.ts`
// and `progress.ts`; the IPC is a `FindingsChannel` (`types.ts`). Every control is Nuxt UI
// (RB-080-88): the filters are `FindingsFilters.vue`, each rule a `UAccordion` item, open until
// the reader folds it, and what the project turned off a `UCollapsible`.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UAccordion from '@nuxt/ui/components/Accordion.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCollapsible from '@nuxt/ui/components/Collapsible.vue';
import ExternalLink from '../links/ExternalLink.vue';
import FindingsFilters from './FindingsFilters.vue';
import FindingsProgress from './FindingsProgress.vue';
import { FINDINGS_ICONS } from './icons';
import {
  SEVERITIES,
  countBySeverity,
  filterFindings,
  groupByRule,
  highlightRequest,
  moveFocus,
  nestMessages,
  onlyIn,
  pluginsOf,
} from './findings';
import type { IgnoreEntry, ProjectOff } from '../ipc/protocol';
import type {
  CheckFailure,
  CheckedView,
  ChecksProgress,
  FindingsChannel,
  FindingsEvent,
  FindingsRun,
  MergedFinding,
  ProgressEvent,
  Severity,
} from './types';

const props = defineProps<{ channel: FindingsChannel }>();
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
const offOpen = ref(false);
const counts = computed(() => countBySeverity(run.value?.findings ?? []));
const plugins = computed(() => (run.value === null ? [] : pluginsOf(run.value)));
const viewChoices = computed(() => (run.value?.views ?? []).map((checked) => ({ id: checked.id, name: viewName(checked) })));

function viewName(checked: CheckedView): string {
  return checked.breakpoint ?? t('findings.view_id', { id: String(checked.id) });
}

function onlyInText(finding: MergedFinding): string | null {
  const views = run.value === null ? null : onlyIn(finding, run.value);
  return views === null ? null : t('findings.only_in', { breakpoints: views.map(viewName).join(', ') });
}

/** An ignore entry as one line: its globs, `*` for one left out. */
function ignoreText(entry: IgnoreEntry): string {
  return [entry.rule, entry.url, entry.selector].map((glob) => glob ?? '*').join(' \u00b7 ');
}

const offCount = computed(() => (projectOff.value === null ? 0 : projectOff.value.rules.length + projectOff.value.ignore.length));

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
  return active === null || active === document.body || active === runButton.value?.$el || (active === cancel && cancel !== null);
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
  if (!(await send({ type: 'runChecks', target: 'all' }))) {
    running.value = false;
  }
}

function cancelChecks(): void {
  void send({ type: 'runCommand', id: 'checks.cancel' });
}

/** A plugin's failure as one sentence: the view, and the reason's text when there is a code. */
function failureText(failure: CheckFailure): string {
  if (failure.code === undefined) {
    return t('findings.plugin_failed', { plugin: failure.plugin });
  }
  const checked = run.value?.views.find((view) => view.id === failure.view);
  const where = checked === undefined ? '' : t('findings.plugin_failed_view', { view: viewName(checked) });
  // The code's own parameters (`{service}`) so none shows raw (RB-080-98); the host's names of
  // the plugin last, so a plugin cannot rename itself, and no raw engine text (`reason`).
  const reason = t(failure.code, { ...failure.params, id: failure.plugin, plugin: failure.plugin, reason: '' });
  return t('findings.plugin_failed_reason', { plugin: failure.plugin, where, reason });
}

function choose(finding: MergedFinding): void {
  selected.value = finding.id;
  const request = highlightRequest(finding);
  if (request !== null) {
    void send(request);
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
        :disabled="running"
        data-testid="run"
        @click="runChecks"
      >
        {{ t('findings.run') }}
      </UButton>
      <UButton size="sm" variant="ghost" data-testid="rules" @click="openRules">
        {{ t('findings.rules_settings') }}
      </UButton>
      <p v-if="run && !running" class="text-sm text-muted" data-testid="summary">
        {{ t('findings.summary_count', { total: run.summary.total, plugins: run.plugins.length }) }}
      </p>
    </header>

    <UCollapsible v-if="offCount > 0 && projectOff" v-model:open="offOpen" class="text-sm text-muted" data-testid="project-off">
      <UButton
        size="sm"
        variant="link"
        color="neutral"
        class="px-0"
        data-testid="project-off-toggle"
        :trailing-icon="FINDINGS_ICONS.checkDetails"
        :ui="{ trailingIcon: offOpen ? 'rotate-180 transition-transform' : 'transition-transform' }"
      >
        {{ t('findings.project_off.note', { count: String(offCount) }) }}
      </UButton>
      <template #content>
        <ul class="mt-1 list-disc pl-5">
          <li v-for="rule in projectOff.rules" :key="`rule-${rule}`">{{ t('findings.project_off.rule', { rule }) }}</li>
          <li v-for="(entry, index) in projectOff.ignore" :key="`ignore-${index}`">
            {{ t('findings.project_off.ignore', { entry: ignoreText(entry) }) }}
          </li>
        </ul>
      </template>
    </UCollapsible>

    <FindingsProgress v-if="progress" :progress="progress" :elapsed-ms="elapsedMs" @cancel="cancelChecks" />
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
