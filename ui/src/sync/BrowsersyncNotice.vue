<script setup lang="ts">
// The notice in the toolbar while Browsersync keeps sync off (RB-020-06): a page runs
// Browsersync, whose ghost mode syncs by itself, so the core switched rBrowser's sync off. The
// notice says why (its title the whole reason) and offers "sync anyway", which switches sync on
// again - for that site, for the rest of the session. It goes as soon as sync is on. Nuxt UI since
// RB-080-88: the notice a warning `UBadge`, "sync anyway" a `UButton`.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
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
    class="flex min-w-0 shrink-0 items-center gap-1"
    :title="t('sync.browsersync.paused', { origin })"
    data-testid="browsersync-notice"
  >
    <UBadge
      size="sm"
      color="warning"
      variant="subtle"
      class="min-w-0"
      :ui="{ label: 'truncate' }"
      :label="t('sync.browsersync.notice')"
      data-testid="browsersync-badge"
    />
    <UButton
      size="xs"
      color="warning"
      variant="soft"
      class="shrink-0"
      data-testid="browsersync-anyway"
      @click="shell.run({ command: 'sync.toggle', enabled: true })"
    >
      {{ t('sync.browsersync.anyway') }}
    </UButton>
  </div>
</template>
