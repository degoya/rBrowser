<script setup lang="ts">
// The page `status-bar.html` (RB-080-15): the status bar in its own CEF view at the bottom of the
// main window. It talks to the core through a client of its own, like the pages window.
import UApp from '@nuxt/ui/components/App.vue';
import type { IpcClient } from '../ipc/client';
import StatusBar from './StatusBar.vue';

defineProps<{ client: IpcClient }>();
</script>

<template>
  <UApp>
    <StatusBar :client="client" />
  </UApp>
</template>
