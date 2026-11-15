<script setup lang="ts">
// The example panel: it calls the plugin's tool `count_views` once its texts arrived and again on
// Refresh, says the count in the status line and shows a badge per view width; loading, empty
// and failed are three different states. The parts:
// `PanelFrame` (title, icon, the header's actions, one status line, the scrolling content),
// `IconAction` for a header action, `usePanel()` for the texts (`t`), a failed call as one
// sentence (`error`) and `call`; everything else is Nuxt UI (https://ui.nuxt.com) - an own
// element only where Nuxt UI has none.
import { computed, ref } from 'vue';
import { usePanel } from '../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../web-common/panel-ui/src/status';
import IconAction from '../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../web-common/panel-ui/src/components/PanelFrame.vue';

interface Answer {
  count: number;
  widths: number[];
}

const panel = usePanel();
const t = panel.t;

const answer = ref<Answer | null>(null);
const failure = ref<string | null>(null);
const busy = ref(false);

async function refresh(): Promise<void> {
  busy.value = true;
  try {
    answer.value = (await panel.call({ tool: 'count_views' })) as Answer;
    failure.value = null;
  } catch (error) {
    failure.value = panel.error(error);
  } finally {
    busy.value = false;
  }
}

// Texts first: the panel counts once they arrived.
panel.onSettings((first) => {
  if (first) void refresh();
});

const line = computed<StatusLine | null>(() => {
  if (failure.value) return { tone: 'error', text: failure.value };
  if (answer.value) return { tone: 'info', text: t('panel.count', { n: answer.value.count }) };
  return null;
});
</script>

<template>
  <PanelFrame :title="t('panels.main')" icon="i-lucide-layout-grid" :status="line">
    <template #actions>
      <IconAction id="refresh" icon="i-lucide-refresh-cw" :label="t('panel.refresh')" :disabled="busy" @click="refresh" />
    </template>

    <UEmpty v-if="!answer && busy" id="loading" icon="i-lucide-loader-circle" :title="t('panel.loading')" variant="naked" :ui="{ avatar: 'animate-spin' }" />
    <UEmpty v-else-if="answer && answer.count === 0" id="empty" icon="i-lucide-layout-grid" :title="t('panel.empty')" variant="naked" />
    <div v-else-if="answer" id="widths" class="flex flex-wrap gap-1.5">
      <UBadge v-for="(width, index) in answer.widths" :key="index" data-width color="neutral" variant="subtle" :label="t('panel.width', { width })" />
    </div>
  </PanelFrame>
</template>
