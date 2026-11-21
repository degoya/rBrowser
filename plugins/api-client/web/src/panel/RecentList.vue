<script setup lang="ts">
// The last sends (RB-096-01), under the collections in the Requests tab: a saved one opens
// again, a free one fills the editor with its method and URL template. Never a secret.
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import type { HistoryEntry } from './model';

defineProps<{ entries: HistoryEntry[] }>();
const emit = defineEmits<{ pick: [entry: HistoryEntry] }>();
const t = usePanel().t;
</script>

<template>
  <section v-if="entries.length > 0" id="history" class="flex flex-col gap-0.5" :aria-label="t('panel.history')">
    <p class="text-xs text-muted">{{ t('panel.history') }}</p>
    <UButton
      v-for="(entry, index) in entries.slice(0, 15)"
      :key="index"
      data-history
      size="xs"
      color="neutral"
      variant="ghost"
      class="justify-start font-normal"
      @click="emit('pick', entry)"
    >
      <span class="w-10 shrink-0 font-mono text-muted">{{ entry.status ?? t('panel.failed') }}</span>
      <span class="min-w-0 truncate font-mono">{{ `${entry.method} ${entry.url}` }}</span>
    </UButton>
  </section>
</template>
