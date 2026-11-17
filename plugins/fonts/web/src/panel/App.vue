<script setup lang="ts">
// The Fonts panel (RB-070-13) on Nuxt UI (RB-080-83), ordered as the owner asked on 2026-11-15:
// one status line on top ("2 views, 3 families; no fonts from mobile"), then two tabs - Fonts:
// the families as a keyboard list with a preview line in each font and their facts as badges
// (arrows move, Enter or a click marks where the family is used in every view), and the selected
// family's detail in sections (preview per face, faces, use per view, findings) that keep their
// open or closed state; Sizes: the font sizes of the text roles per breakpoint, with "not scaled"
// and "jumps" in a column of their own. A web font's file comes from the tool `preview`, never
// from the network (preview.ts).
import { computed, ref, watch } from 'vue';
import { answerBudget } from '../../../../web-common/panel-ui/src/limit';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine, Tone } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import FamilyDetail from './FamilyDetail.vue';
import FamilyList from './FamilyList.vue';
import SizesTable from './SizesTable.vue';
import { type Answer, rows } from './model';
import { browserLoad, FontPreviews, type PreviewPart } from './preview';

const panel = usePanel();
const t = panel.t;

const answer = ref<Answer>();
const loading = ref(false);
const selected = ref(0);
const message = ref<StatusLine | null>(null);
const tab = ref('fonts');
/** The detail's open sections, kept from family to family. */
const open = ref<string[]>(['preview', 'faces']);

const previews = new FontPreviews({
  read: (view, src, offset) => panel.call({ tool: 'preview' }, { view, src, offset }) as Promise<PreviewPart>,
  load: browserLoad,
});

const list = computed(() => (answer.value ? rows(answer.value) : []));
const family = computed(() => answer.value?.families.find((one) => one.family === list.value[selected.value]?.family));

const say = (text: string, tone: Tone = 'info'): void => {
  message.value = { tone, text };
};
const failed = (error: unknown): void => say(panel.error(error), 'error');

/** The status line: what the last action said, the host's waiting, else what the views hold. */
const line = computed<StatusLine | null>(() => {
  if (panel.waiting.value) return { tone: 'busy', text: panel.strings.value['plugins.panel.waiting'] ?? '' };
  if (message.value) return message.value;
  const now = answer.value;
  if (!now) return null;
  const parts = [t('panel.summary', { views: now.views.length, families: now.families.length })];
  const unsupported = now.views.filter((view) => view.error).map((view) => view.label);
  if (unsupported.length) parts.push(t('panel.unsupported', { views: unsupported.join(', ') }));
  if (now.cut) parts.push(t('panel.cut'));
  return { tone: 'info', text: parts.join(' ') };
});

/** Reads the views. The loading state shows on the first read only; a read again keeps what
 * the panel shows until the answer is in, and a failure is said in the status line, never as
 * loading or empty (docs/design.md 5.6). */
function load(): void {
  loading.value = true;
  panel
    .call({ tool: 'list' }, { budget: answerBudget(panel.settings.value) })
    .then((reply) => {
      answer.value = reply as Answer;
      selected.value = 0;
      message.value = null;
    })
    .catch(failed)
    .finally(() => {
      loading.value = false;
    });
}

function select(index: number): void {
  const count = list.value.length;
  if (count === 0) return;
  selected.value = Math.max(0, Math.min(count - 1, index));
}

function mark(): void {
  const row = list.value[selected.value];
  if (!row) return;
  panel
    .call({ tool: 'mark' }, { family: row.family })
    .then((reply) => {
      const marked = (reply as { marked?: { count?: number }[] } | null)?.marked ?? [];
      const total = marked.reduce((sum, entry) => sum + (entry.count ?? 0), 0);
      say(t('panel.marked', { count: total, views: marked.filter((entry) => (entry.count ?? 0) > 0).length }));
    })
    .catch(failed);
}

function check(): void {
  panel
    .call({ command: 'check' })
    .then((reply) => say(t('panel.checked', { count: (reply as { findings?: number } | null)?.findings ?? 0 }), 'ok'))
    .catch(failed);
}

function exportList(): void {
  panel
    .call({ tool: 'export' })
    .then((reply) => say(t('panel.exported', { files: ((reply as { files?: string[] } | null)?.files ?? []).join(', ') }), 'ok'))
    .catch(failed);
}

function clear(): void {
  panel.call({ tool: 'mark' }, { clear: true }).catch(failed);
}

// Every family's file is read once, in the list's order.
watch(answer, (now) => {
  if (now) previews.request(now, list.value.map((row) => now.families.find((one) => one.family === row.family)).filter((one) => one !== undefined));
});

const tabs = computed(() => [
  { label: t('panel.tabFonts'), value: 'fonts', slot: 'fonts' as const, icon: 'i-lucide-type' },
  { label: t('panel.tabSizes'), value: 'sizes', slot: 'sizes' as const, icon: 'i-lucide-ruler' },
]);

// Asks once its settings came (RB-090-44): they name the message limit one answer may fill.
panel.onSettings((first) => {
  if (first) load();
});
</script>

<template>
  <PanelFrame :title="t('panels.fonts')" icon="i-lucide-type" :status="line">
    <template #actions>
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" @click="load" />
      <IconAction id="export" icon="i-lucide-download" :label="t('panel.export')" @click="exportList" />
      <IconAction id="clear" icon="i-lucide-eraser" :label="t('panel.clear')" @click="clear" />
      <IconAction id="check" icon="i-lucide-scan-search" :label="t('panel.check')" primary @click="check" />
    </template>

    <div v-if="!answer" id="state" role="status">
      <UEmpty v-if="loading" icon="i-lucide-loader-circle" :title="t('panel.loading')" variant="naked" :ui="{ avatar: 'animate-spin' }" />
    </div>
    <div v-else-if="list.length === 0" id="state" role="status">
      <UEmpty icon="i-lucide-type" :title="t('panel.empty')" variant="naked" />
    </div>
    <PanelTabs v-else id="tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm">
      <template #fonts>
        <div id="split" class="grid gap-3 pt-2 md:grid-cols-[minmax(220px,2fr)_3fr]">
          <FamilyList :answer="answer" :rows="list" :selected="selected" :previews="previews" @select="select" @mark="mark" />
          <FamilyDetail v-if="family" v-model:open="open" :answer="answer" :family="family" :previews="previews" />
        </div>
      </template>
      <template #sizes>
        <div class="pt-2">
          <SizesTable :answer="answer" />
        </div>
      </template>
    </PanelTabs>
  </PanelFrame>
</template>
