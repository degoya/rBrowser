<script setup lang="ts">
// The toolbar's sitemap indicator (RB-050-05): shown only while a capture runs, with its shots
// done of all and the time left in its title. A native, Tab-reachable button; it opens the
// sitemap bar (`panels.sitemap`), where the run is cancelled or its gallery opened.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import { useShell } from '../state/shell';
import { PANEL_COMMAND, isRunning, sitemapOf, stateKey, timeLeft } from './sitemap';

const shell = useShell();
const { t } = useI18n();

const status = computed(() => sitemapOf(shell.state.value));
const running = computed(() => isRunning(status.value));
const args = computed(() => ({
  done: status.value?.done ?? 0,
  total: status.value?.total ?? 0,
  failed: status.value?.failed ?? 0,
}));
const title = computed(() => {
  const left = timeLeft(status.value?.etaSeconds);
  const line = `${t('shell.sitemap.label')}: ${t(stateKey(status.value), args.value)}`;
  return left === null ? line : `${line}, ${t(left.key, left.args)}`;
});

function open(): void {
  void shell.send({ type: 'runCommand', id: PANEL_COMMAND });
}
</script>

<template>
  <UButton
    v-if="running"
    color="primary"
    variant="soft"
    size="sm"
    class="max-w-40 shrink-0 truncate"
    :label="t('shell.sitemap.indicator', args)"
    :title="title"
    data-testid="sitemap-indicator"
    @click="open"
  />
</template>
