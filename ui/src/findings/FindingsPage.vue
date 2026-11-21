<script setup lang="ts">
// The findings panel (RB-060-01) in the plugin panels page (RB-080-69: no window of its own any
// more): `panels.findings` (its leader chord, the palette, the context menu) and a finished check
// open the panels page - floating or docked - at Findings (`PanelsPage.vue`, `core.ts`), so the
// shell strip keeps its 44 px. On load it asks for the last run (`getFindings`), so a run that
// ended before the page showed it (`checks.run_all` from a shortcut or MCP) is there, and the
// focus goes to Run.
import { nextTick, onMounted, ref } from 'vue';
import type { IpcClient } from '../ipc/client';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';
import FindingsPanel from './FindingsPanel.vue';
import { findingsChannel } from './host';

// The pages window hands every page its route key and the trust state as well.
defineOptions({ inheritAttrs: false });
const props = withDefaults(defineProps<{ client: IpcClient; subscribe?: Subscribe }>(), {
  subscribe: undefined,
});
const shell = useShell();
const channel = findingsChannel(
  shell.send,
  props.subscribe ?? windowEvents,
  (message) => props.client.request(message),
  (command) => shell.bindingOf(command),
);
const host = ref<HTMLElement | null>(null);

onMounted(() => {
  void nextTick(() => host.value?.querySelector<HTMLElement>('[data-testid="run"]')?.focus());
});
</script>

<template>
  <div ref="host" class="h-full overflow-auto bg-default text-default" data-testid="findings-page">
    <FindingsPanel :channel="channel" />
  </div>
</template>
