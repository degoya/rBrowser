<script setup lang="ts">
// The Sessions tab (RB-080-37, on Nuxt UI since RB-080-84): Save session writes the capture as
// HAR; the saved sessions, each with Open, which shows it in the Requests tab for display.
import { useNetwork } from './store';

const net = useNetwork();
const sessions = net.sessions;
const t = net.panel.t;
</script>

<template>
  <div id="panel-sessions" class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <h2 id="sessions-title" class="flex-1 text-sm font-semibold text-highlighted">{{ t('sessions.title') }}</h2>
      <UButton id="session-save" size="sm" icon="i-lucide-save" :label="t('sessions.save')" @click="sessions.save()" />
    </div>
    <p v-if="sessions.note.value" id="sessions-note" role="status" class="text-sm text-muted">{{ sessions.note.value }}</p>
    <ul v-if="sessions.list.value.length" id="sessions" class="divide-y divide-default rounded-md border border-default">
      <li v-for="session in sessions.list.value" :key="session.path" data-session class="flex items-center gap-2 px-2 py-1.5">
        <UIcon name="i-lucide-file-clock" class="size-4 shrink-0 text-muted" />
        <span data-name class="min-w-0 flex-1 truncate font-mono text-xs text-highlighted" :title="session.path">{{ session.name }}</span>
        <UButton size="xs" variant="outline" color="neutral" data-open :label="t('sessions.open')" @click="sessions.open(session.path)" />
      </li>
    </ul>
    <UEmpty v-else id="sessions-empty" icon="i-lucide-history" :title="t('sessions.none')" variant="naked" />
  </div>
</template>
