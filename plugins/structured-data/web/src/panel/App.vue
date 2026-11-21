<script setup lang="ts">
// The structured data panel (manifest [[panels]], RB-090-96): the schema.org data of the page the
// chosen view shows (the active view by default) in three tabs - the tree, the findings (a click
// shows the finding's node in the tree), the raw JSON-LD blocks. "Read again" calls the tool
// `inspect` for the chosen view, so it follows the view; "Send to the findings panel" runs the
// command `check`, whose `checked` event then starts no second read.
import { computed, nextTick, onBeforeUnmount, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { answerBudget } from '../../../../web-common/panel-ui/src/limit';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import FindingList from './FindingList.vue';
import RawBlocks from './RawBlocks.vue';
import StructureTree from './StructureTree.vue';
import { ancestorsOf, type Answer, type Finding, SECTIONS, type Section, statusOf, text, toTree, type TreeItem, viewItems } from './model';

const SECTION_ICON: Record<Section, string> = {
  tree: 'i-lucide-list-tree',
  findings: 'i-lucide-triangle-alert',
  raw: 'i-lucide-braces',
};

const panel = usePanel();
const strings = panel.strings;
const tx = (key: string, params?: Record<string, string | number>): string => text(strings.value, key, params);

const answer = ref<Answer | null>(null);
const loading = ref(false);
const failure = ref<string | null>(null);
const reported = ref<number | null>(null);
const tab = ref<Section>('tree');
const chosen = ref<number | 'active'>('active');
const expanded = ref<string[]>([]);
/** The panel's own report runs the command, whose answer posts `checked`: no second read. */
let reporting = false;

const tree = computed<TreeItem[]>(() => (answer.value ? toTree(strings.value, answer.value) : []));
const views = computed(() => viewItems(strings.value, answer.value));
const tabs = computed(() =>
  SECTIONS.map((name) => ({
    label: tx(`panel.sections.${name}`),
    value: name,
    slot: name,
    icon: SECTION_ICON[name],
    badge: name === 'findings' && answer.value ? String(answer.value.findings.length) : undefined,
  })),
);
const line = computed(() => {
  if (panel.waiting.value) return { tone: 'busy' as const, text: strings.value['plugins.panel.waiting'] ?? '' };
  if (reported.value !== null && !failure.value && !loading.value) return { tone: 'ok' as const, text: tx('panel.reported', { count: reported.value }) };
  return statusOf(strings.value, { failure: failure.value, loading: loading.value, answer: answer.value });
});

/** The keys of every entry open by default. */
function opened(items: TreeItem[]): string[] {
  return items.flatMap((item) => [...(item.defaultExpanded ? [item.key] : []), ...opened(item.children ?? [])]);
}

function load(): void {
  loading.value = true;
  failure.value = null;
  reported.value = null;
  const args: Record<string, unknown> = { budget: answerBudget(panel.settings.value) };
  if (chosen.value !== 'active') args.target = chosen.value;
  panel
    .call({ tool: 'inspect' }, args)
    .then(
      (value) => {
        answer.value = value as Answer;
        expanded.value = opened(tree.value);
      },
      (error: unknown) => {
        failure.value = panel.error(error);
      },
    )
    .finally(() => {
      loading.value = false;
    });
}

function report(): void {
  reporting = true;
  failure.value = null;
  panel
    .call({ command: 'check' }, {})
    .then(
      (value) => {
        reported.value = (value as { findings?: number } | null)?.findings ?? 0;
      },
      (error: unknown) => {
        failure.value = panel.error(error);
      },
    )
    .finally(() => {
      reporting = false;
    });
}

function choose(value: unknown): void {
  chosen.value = value === 'active' ? 'active' : Number(value);
  load();
}

/** Focuses the node's entry once the tree shows it (the tab's content appears a frame later). */
function focusWhenShown(id: string, frames = 10): void {
  const target = document.getElementById(id);
  if (target && target.offsetParent !== null) {
    target.scrollIntoView?.({ block: 'center' });
    (target.closest('[role="treeitem"]') as HTMLElement | null)?.focus();
  } else if (frames > 0) {
    requestAnimationFrame(() => focusWhenShown(id, frames - 1));
  }
}

/** Shows the node `id` in the tree: its tab, its ancestors opened, the entry focused. */
function show(id: string): void {
  tab.value = 'tree';
  const above = ancestorsOf(tree.value, id) ?? [];
  expanded.value = [...new Set([...expanded.value, ...above, id])];
  void nextTick(() => focusWhenShown(`node-${id}`));
}

function jump(finding: Finding): void {
  if (finding.node) show(finding.node);
}

// The command `check` posts `checked`; the panel's own report starts no second read.
const stopEvents = onPanelEvent((message) => {
  if ((message as { type?: unknown } | null)?.type === 'checked' && !reporting) load();
});
onBeforeUnmount(stopEvents);

panel.onSettings((first) => {
  document.title = tx('panels.structured-data');
  if (first) load();
});
</script>

<template>
  <PanelFrame :title="tx('panels.structured-data')" icon="i-lucide-code" :status="line">
    <template #actions>
      <IconAction id="report" icon="i-lucide-list-checks" :label="tx('panel.report')" :disabled="!answer" @click="report" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="tx('panel.refresh')" primary :disabled="loading" @click="load" />
    </template>

    <div class="flex flex-col gap-3">
      <div class="flex flex-wrap items-center gap-2 text-sm text-muted">
        <USelect id="view" class="min-w-0 flex-1" size="sm" :items="views" :model-value="chosen" :aria-label="tx('panel.view')" @update:model-value="choose" />
        <span v-if="answer" id="vocabulary" class="flex items-center gap-1.5">
          <UIcon name="i-lucide-book-open" class="size-4 shrink-0" />
          <span>{{ tx('panel.vocabulary', { version: answer.vocabulary }) }}</span>
        </span>
      </div>
      <p v-if="answer" id="url" class="text-xs break-all text-muted">{{ answer.url }}</p>

      <UEmpty v-if="!loading && !failure && answer && answer.items.length === 0 && answer.blocks.length === 0" id="empty" icon="i-lucide-code" :title="tx('panel.empty')" variant="naked" />

      <PanelTabs v-else-if="answer" id="tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm">
        <template #tree>
          <section id="tree-section" tabindex="-1" class="pt-2 outline-none">
            <StructureTree v-model:expanded="expanded" :items="tree" @show="show" />
          </section>
        </template>
        <template #findings>
          <section id="findings" tabindex="-1" class="pt-2 outline-none">
            <FindingList :answer="answer" @jump="jump" />
          </section>
        </template>
        <template #raw>
          <section id="raw" tabindex="-1" class="pt-2 outline-none">
            <RawBlocks :blocks="answer.blocks" />
          </section>
        </template>
      </PanelTabs>
    </div>
  </PanelFrame>
</template>
