<script setup lang="ts">
// The responsive images panel (manifest [[panels]], RB-093-02): a tab per view - its breakpoint,
// its density and its count of problems - with a table of its images (ImageTable.vue). "Check
// again" calls the tool `check` for every view; "Send to the findings panel" runs the command
// `check`, whose `checked` event then starts no second read. Hovering an image (or focusing its
// name) marks it in every view through the tool `mark`; leaving the table clears the marks.
import { computed, onBeforeUnmount, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { answerBudget } from '../../../../web-common/panel-ui/src/limit';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import ImageTable from './ImageTable.vue';
import { type Answer, createMarker, markTargets, statusOf, tabsOf, text } from './model';

const panel = usePanel();
const strings = panel.strings;
const tx = (key: string, params?: Record<string, string | number>): string => text(strings.value, key, params);

const answer = ref<Answer | null>(null);
const loading = ref(false);
const failure = ref<string | null>(null);
const reported = ref<number | null>(null);
const tab = ref<string | undefined>(undefined);
const onlyProblems = ref(false);
/** The panel's own report runs the command, whose answer posts `checked`: no second read. */
let reporting = false;

const tabs = computed(() => tabsOf(strings.value, answer.value));
const line = computed(() => {
  if (panel.waiting.value) return { tone: 'busy' as const, text: strings.value['plugins.panel.waiting'] ?? '' };
  if (reported.value !== null && !failure.value && !loading.value) return { tone: 'ok' as const, text: tx('panel.reported', { count: reported.value }) };
  return statusOf(strings.value, { failure: failure.value, loading: loading.value, answer: answer.value });
});

const marker = createMarker((selector) =>
  selector === null ? panel.call({ tool: 'mark' }, { clear: true }) : panel.call({ tool: 'mark' }, { targets: markTargets(answer.value, selector) }),
);

function load(): void {
  loading.value = true;
  failure.value = null;
  reported.value = null;
  panel
    .call({ tool: 'check' }, { budget: answerBudget(panel.settings.value) })
    .then(
      (value) => {
        answer.value = value as Answer;
        const ids = answer.value.views.map((view) => String(view.id));
        if (!tab.value || !ids.includes(tab.value)) tab.value = ids[0];
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

function failedText(view: { error?: { code: string; params?: Record<string, string> } }): string {
  return tx('panel.failed', { reason: panel.error(Object.assign(new Error(view.error?.code ?? ''), { params: view.error?.params ?? {} })) });
}

// The command `check` posts `checked`; the panel's own report starts no second read.
const stopEvents = onPanelEvent((message) => {
  if ((message as { type?: unknown } | null)?.type === 'checked' && !reporting) load();
});
onBeforeUnmount(() => {
  stopEvents();
  marker.stop();
});

panel.onSettings((first) => {
  document.title = tx('panels.responsive-images');
  if (first) load();
});
</script>

<template>
  <PanelFrame :title="tx('panels.responsive-images')" icon="i-lucide-image" :status="line">
    <template #actions>
      <IconAction id="report" icon="i-lucide-list-checks" :label="tx('panel.report')" :disabled="!answer" @click="report" />
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="tx('panel.refresh')" primary :disabled="loading" @click="load" />
    </template>

    <div class="flex flex-col gap-3">
      <USwitch id="only-problems" v-model="onlyProblems" size="sm" :label="tx('panel.onlyProblems')" />

      <UEmpty v-if="!loading && !failure && answer && answer.views.length === 0" id="no-view" icon="i-lucide-image" :title="tx('panel.noView')" variant="naked" />

      <PanelTabs v-else-if="answer" id="tabs" v-model="tab" :items="tabs" :unmount-on-hide="false" variant="link" size="sm">
        <template v-for="item in tabs" :key="item.value" #[item.slot]>
          <p v-if="item.view.error" data-failed class="flex items-center gap-1.5 pt-2 text-sm text-error">
            <UIcon name="i-lucide-circle-x" class="size-4 shrink-0" />
            <span>{{ failedText(item.view) }}</span>
          </p>
          <ImageTable v-else :view="item.view" :only-problems="onlyProblems" @mark="marker.hover" />
        </template>
      </PanelTabs>
    </div>
  </PanelFrame>
</template>
