<script setup lang="ts">
// The page `status-bar.html` (RB-080-15): the status bar in its own CEF view at the bottom of the
// main window. It talks to the core through a client of its own, like the pages window. Above
// the bar the activity bar while it stands at the bottom (RB-090-67): the core makes the view
// that much higher, and keeps it shown for the activity bar alone while the status bar itself is
// hidden (`ui.statusBar.visible`).
import { computed } from 'vue';
import UApp from '@nuxt/ui/components/App.vue';
import ActivityBar from '../activity/ActivityBar.vue';
import type { IpcClient } from '../ipc/client';
import { useShell } from '../state/shell';
import StatusBar from './StatusBar.vue';

defineProps<{ client: IpcClient }>();
const shell = useShell();
const statusRow = computed(() => shell.settings.value['ui.statusBar.visible'] !== false);
</script>

<template>
  <UApp>
    <ActivityBar place="bottom" />
    <StatusBar v-if="statusRow" :client="client" />
  </UApp>
</template>
