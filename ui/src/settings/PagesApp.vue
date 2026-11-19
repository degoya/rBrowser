<script setup lang="ts">
// The pages window (`settings.html`): one window beside the main one for everything larger than
// the toolbar - the settings, the trust dialog (RB-010-09), the analyser's wizard (RB-010-13).
// The route is the URL hash; the core opens a page by loading its URL.
import { computed } from 'vue';
import UApp from '@nuxt/ui/components/App.vue';
import type { IpcClient } from '../ipc/client';
import { useShell } from '../state/shell';
import { pageOf } from './routes';
import { useHashRoute } from './route';

defineProps<{ client: IpcClient }>();
const shell = useShell();
const route = useHashRoute();
const page = computed(() => pageOf(route.value.page));
const trusted = computed(() => shell.state.value?.project?.trusted ?? null);
/** Whether a project is open; `null` until the first state (the analyser's wizard, RB-090-43). */
const projectOpen = computed(() => (shell.state.value === null ? null : shell.state.value.project !== null));
</script>

<template>
  <UApp>
    <component :is="page" :client="client" :route-key="route.key" :route-tab="route.tab" :route-section="route.section ?? null" :trusted="trusted" :project-open="projectOpen" />
  </UApp>
</template>
