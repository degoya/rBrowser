<script setup lang="ts">
// The outline panel (manifest [[panels]], RB-070-12; on Nuxt UI since RB-080-85): a tab per
// section, its entries as a tree with a dot per view and "only <views>" where the views differ,
// the findings at each entry; the link check in batches of `outline.linkCheck.rps` (a panel call
// must answer within 10 s); selecting an entry marks it in every view. The owner's word on
// Outline (2026-11-15): sections as tabs, Refresh and Clear marks set apart as header actions -
// Refresh a ghost icon button, Clear marks a soft red one, enabled only while something is
// marked -, one status line with an icon, an empty section as `UEmpty`. RB-091-03: each tab
// counts its findings (from one findings-only `get` until the tab is read), coloured by the
// worst; the findings at no entry (the page's, content outside the landmarks) stand above the
// tree, a click marks them; "Only problems" keeps the entries with a finding.
import { computed, reactive, ref } from 'vue';
import { answerBudget } from '../../../../web-common/panel-ui/src/limit';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import OutlineTree from './OutlineTree.vue';
import {
  type Answer,
  batches,
  distinct,
  findingsOf,
  type Item,
  items,
  loose,
  type MarkTarget,
  problemsOnly,
  SEVERITY_ICON,
  statusLine,
  type Tab,
  TABS,
  tabBadge,
  text,
} from './model';

const TAB_ICON: Record<Tab, string> = {
  headings: 'i-lucide-heading',
  landmarks: 'i-lucide-map',
  links: 'i-lucide-link',
  images: 'i-lucide-image',
  forms: 'i-lucide-text-cursor-input',
  focus: 'i-lucide-focus',
  axtree: 'i-lucide-accessibility',
  custom: 'i-lucide-scan-search',
};

const panel = usePanel();
const strings = panel.strings;
const tx = (key: string, params?: Record<string, string | number>): string => text(strings.value, key, params);

const tab = ref<Tab>('headings');
const axView = ref<number | undefined>(undefined);
const cache = reactive(new Map<Tab, Answer>());
const summary = ref<Answer | null>(null);
const onlyProblems = ref(false);
/** The tabs whose entries carry findings ("Only problems" and the findings above the tree). */
const CHECKED: Tab[] = ['headings', 'landmarks', 'links', 'images', 'forms', 'focus'];
const linkStates = reactive(new Map<string, { state: string; status?: number }>());
const failure = ref<string | null>(null);
const marked = ref<{ count: number; views: number } | null>(null);
const progress = ref<{ done: number; total: number } | null>(null);
const checking = ref(false);
let stop = false;

const answer = computed(() => cache.get(tab.value));
const checked = computed(() => CHECKED.includes(tab.value));
const all = computed<Item[]>(() => (answer.value ? items(strings.value, tab.value, answer.value, axView.value) : []));
const list = computed<Item[]>(() => (onlyProblems.value && checked.value ? problemsOnly(all.value) : all.value));
const pageNotes = computed(() => (answer.value && checked.value ? loose(strings.value, tab.value, answer.value) : []));
const axItems = computed(() => (answer.value?.views ?? []).map((view) => ({ label: view.label, value: view.id })));
const linkCheck = computed(() => tab.value === 'links' && panel.settings.value['outline.linkCheck.enabled'] !== false && !!answer.value);
const tabs = computed(() =>
  TABS.map((name) => {
    const source = cache.get(name) ?? summary.value;
    return { label: tx(`panel.tabs.${name}`), value: name, icon: TAB_ICON[name], badge: source ? tabBadge(findingsOf(source, name)) : undefined };
  }),
);

const line = computed(() => {
  const current = answer.value;
  const without = (current?.views ?? []).filter((view) => view.error).map((view) => view.label);
  const cut = !!current && ((current as { cut?: boolean }).cut || JSON.stringify(current.sections ?? {}).includes('"cut":true'));
  return statusLine([
    failure.value ? { tone: 'error', text: failure.value } : null,
    !current && !failure.value ? { tone: 'busy', text: tx('panel.loading') } : null,
    panel.waiting.value ? { tone: 'busy', text: strings.value['plugins.panel.waiting'] ?? '' } : null,
    progress.value ? { tone: checking.value ? 'busy' : 'info', text: tx('panel.checked', progress.value) } : null,
    marked.value ? { tone: marked.value.count > 0 ? 'ok' : 'warn', text: tx('panel.marked', marked.value) } : null,
    without.length > 0 ? { tone: 'warn', text: tx('panel.noOutline', { views: without.join(', ') }) } : null,
    current && findingsOf(current, tab.value).length > 0 ? { tone: 'info', text: tx('panel.findings', { count: distinct(findingsOf(current, tab.value)).length }) } : null,
    cut ? { tone: 'info', text: tx('panel.cut') } : null,
  ]);
});

function load(name: Tab): void {
  panel.call({ tool: 'get' }, { sections: [name], budget: answerBudget(panel.settings.value) }).then(
    (value) => {
      const typed = value as Answer;
      cache.set(name, typed);
      if (name === 'axtree' && axView.value === undefined) axView.value = typed.views[0]?.id;
    },
    (error: unknown) => {
      failure.value = panel.error(error);
    },
  );
}

/** Every section's findings at once, for the counts on the tabs. */
function loadSummary(): void {
  panel.call({ tool: 'get' }, { findingsOnly: true, budget: answerBudget(panel.settings.value) }).then(
    (value) => {
      summary.value = value as Answer;
    },
    () => {
      // The counts wait for the tabs; the tab's own call says what failed.
    },
  );
}

function select(name: string | number): void {
  tab.value = name as Tab;
  failure.value = null;
  if (!cache.has(tab.value)) load(tab.value);
}

function refresh(): void {
  cache.delete(tab.value);
  linkStates.clear();
  failure.value = null;
  progress.value = null;
  load(tab.value);
  loadSummary();
}

function activate(item: Item): void {
  mark(item.targets);
}

function mark(targets: MarkTarget[]): void {
  if (targets.length === 0) return;
  panel.call({ tool: 'mark' }, { targets }).then(
    (value) => {
      const count = ((value as { marked?: { found?: boolean }[] } | null)?.marked ?? []).filter((one) => one.found).length;
      marked.value = { count, views: targets.length };
    },
    (error: unknown) => {
      failure.value = panel.error(error);
    },
  );
}

function clear(): void {
  panel.call({ tool: 'mark' }, { clear: true }).then(
    () => {
      marked.value = null;
    },
    (error: unknown) => {
      failure.value = panel.error(error);
    },
  );
}

async function checkLinks(): Promise<void> {
  if (checking.value) {
    stop = true;
    return;
  }
  const current = cache.get('links');
  if (!current) return;
  const setting = panel.settings.value['outline.linkCheck.rps'];
  const urls = batches(items(strings.value, 'links', current).map((item) => item.href), typeof setting === 'number' ? setting : 2);
  const total = urls.reduce((sum, batch) => sum + batch.length, 0);
  checking.value = true;
  stop = false;
  failure.value = null;
  progress.value = { done: 0, total };
  try {
    for (const [at, batch] of urls.entries()) {
      if (stop) break;
      // A call keeps the pace inside its batch; a second between batches keeps it across them.
      if (at > 0) await new Promise((done) => setTimeout(done, 1000));
      const reply = (await panel.call({ tool: 'check_links' }, { urls: batch })) as { results?: { url: string; state: string; status?: number }[] };
      for (const result of reply.results ?? []) linkStates.set(result.url, { state: result.state, status: result.status });
      progress.value = { done: (progress.value?.done ?? 0) + batch.length, total };
    }
  } catch (error) {
    failure.value = panel.error(error);
  } finally {
    checking.value = false;
  }
}

panel.onSettings((first) => {
  if (first) {
    load(tab.value);
    loadSummary();
  }
});
</script>

<template>
  <PanelFrame :title="tx('panels.outline')" icon="i-lucide-list-tree" :status="line">
    <template #actions>
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="tx('panel.refresh')" @click="refresh" />
      <UTooltip :text="tx('panel.clear')">
        <UButton
          id="clear"
          icon="i-lucide-eraser"
          color="error"
          variant="soft"
          size="sm"
          :aria-label="tx('panel.clear')"
          :disabled="!marked || marked.count === 0"
          @click="clear"
        />
      </UTooltip>
    </template>

    <PanelTabs
      id="tabs"
      :model-value="tab"
      :items="tabs"
      :content="false"
      variant="link"
      size="sm"
      @update:model-value="select"
    />

    <div id="panel" role="tabpanel" class="flex flex-col gap-2 pt-2">
      <div v-if="tab === 'axtree' && axItems.length > 0" class="flex items-center gap-2">
        <USelect
          id="ax-view"
          size="sm"
          :items="axItems"
          :model-value="axView"
          :aria-label="tx('panel.axView')"
          class="min-w-40"
          @update:model-value="(value: unknown) => (axView = Number(value))"
        />
      </div>
      <div v-if="checked" class="flex items-center gap-2">
        <USwitch id="problems" v-model="onlyProblems" size="sm" :label="tx('panel.onlyProblems')" />
      </div>
      <ul v-if="pageNotes.length > 0" id="page-findings" class="flex flex-col gap-1" :aria-label="tx('panel.pageFindings')">
        <li v-for="note in pageNotes" :key="note.text">
          <UButton
            data-page-note
            :data-severity="note.severity"
            :color="note.severity"
            variant="soft"
            size="sm"
            :icon="SEVERITY_ICON[note.severity]"
            :label="note.text"
            :disabled="note.targets.length === 0"
            class="w-full text-left whitespace-normal"
            @click="mark(note.targets)"
          />
        </li>
      </ul>
      <div v-if="linkCheck" class="flex items-center gap-2">
        <UButton
          id="check-links"
          size="sm"
          :color="checking ? 'neutral' : 'primary'"
          variant="subtle"
          :icon="checking ? 'i-lucide-circle-stop' : 'i-lucide-link-2'"
          :label="checking ? tx('panel.stop') : tx('panel.checkLinks')"
          @click="checkLinks"
        />
      </div>

      <UEmpty
        v-if="answer && list.length === 0"
        id="empty"
        :icon="onlyProblems && checked ? 'i-lucide-circle-check' : TAB_ICON[tab]"
        :title="onlyProblems && checked && all.length > 0 ? tx('panel.noProblems') : tx('panel.empty')"
        variant="naked"
      />
      <OutlineTree v-else-if="answer" :items="list" :links="linkStates" @activate="activate" />
    </div>
  </PanelFrame>
</template>
