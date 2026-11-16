<script setup lang="ts">
// The panel of hello-panel (RB-060-12), on Nuxt UI since RB-080-87 and the plugin SDK's worked
// example of a Nuxt UI panel (docs/plugins.md, "Panels"): `PanelFrame` with the title and the
// one action, the status line, then the content. `Greet every view` calls the tool `greet`; the
// component answers and posts `{ greeted }` to the panel, which says it in the status line. Every
// page that says hello - the page script's `{ loaded }` and the component's `{ page }` - is a row
// of the table. A failed call is one sentence (`panel.error`), never its code.
import { computed, onBeforeUnmount, ref } from 'vue';
import { onPanelEvent } from '../../../../web-common/panel-ui/src/events';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { StatusLine } from '../../../../web-common/panel-ui/src/status';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelFrame from '../../../../web-common/panel-ui/src/components/PanelFrame.vue';
import { readEvent, type Page } from './model';

const panel = usePanel();
const t = panel.t;

const pages = ref<Page[]>([]);
const greeted = ref<string | null>(null);
const failure = ref<string | null>(null);
const busy = ref(false);

const stop = onPanelEvent((message, view) => {
  const event = readEvent(message, view);
  if (event === null) return;
  if ('greeted' in event) greeted.value = event.greeted;
  else pages.value = [...pages.value, event.page];
});
onBeforeUnmount(stop);

async function greet(): Promise<void> {
  busy.value = true;
  failure.value = null;
  try {
    await panel.call({ tool: 'greet' }, {});
  } catch (error) {
    failure.value = panel.error(error);
  } finally {
    busy.value = false;
  }
}

const line = computed<StatusLine | null>(() => {
  if (failure.value) return { tone: 'error', text: failure.value };
  if (greeted.value !== null) return { tone: 'ok', text: t('panel.greeted', { name: greeted.value }) };
  return null;
});

const columns = computed(() => [
  { accessorKey: 'view', header: t('panel.columns.view'), meta: { class: { td: 'tabular-nums' } } },
  { accessorKey: 'url', header: t('panel.columns.page'), meta: { class: { td: 'break-all whitespace-normal' } } },
]);
const rows = computed(() => pages.value.map((page) => ({ view: page.view ?? '', url: page.url })));
</script>

<template>
  <PanelFrame :title="t('panels.main')" icon="i-lucide-hand" :status="line">
    <template #actions>
      <IconAction id="greet" icon="i-lucide-hand" :label="t('panel.greet')" primary :disabled="busy" @click="greet" />
    </template>

    <UEmpty v-if="pages.length === 0" id="empty" icon="i-lucide-hand" :title="t('panel.empty')" variant="naked" />
    <section v-else aria-labelledby="count">
      <p id="count" class="mb-1 text-xs text-muted">{{ t('panel.pages', { n: pages.length }) }}</p>
      <UTable id="pages" :data="rows" :columns="columns" class="text-sm" />
    </section>
  </PanelFrame>
</template>
