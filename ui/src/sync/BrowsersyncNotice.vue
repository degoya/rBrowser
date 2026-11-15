<script setup lang="ts">
// The notice in the toolbar while Browsersync keeps sync off (RB-020-06): a page runs
// Browsersync, whose ghost mode syncs by itself, so the core switched rBrowser's sync off. The
// notice says why (its title the whole reason) and offers "sync anyway", which switches sync on
// again - for that site, for the rest of the session. It goes as soon as sync is on.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { useShell } from '../state/shell';

const shell = useShell();
const { t } = useI18n();

const origin = computed(() => {
  const sync = shell.state.value?.sync;
  return sync !== undefined && !sync.enabled ? (sync.browsersync ?? null) : null;
});
</script>

<template>
  <div
    v-if="origin !== null"
    role="status"
    class="flex shrink-0 items-center gap-1 rounded-sm bg-warning/15 px-1 text-xs text-warning"
    :title="t('sync.browsersync.paused', { origin })"
    data-testid="browsersync-notice"
  >
    <span class="truncate">{{ t('sync.browsersync.notice') }}</span>
    <button
      type="button"
      class="shrink-0 rounded-sm font-medium underline focus-visible:outline-2 focus-visible:outline-primary"
      data-testid="browsersync-anyway"
      @click="shell.run({ command: 'sync.toggle', enabled: true })"
    >
      {{ t('sync.browsersync.anyway') }}
    </button>
  </div>
</template>
