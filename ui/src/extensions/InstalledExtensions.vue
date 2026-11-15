<script setup lang="ts">
// The extensions installed from the Chrome Web Store with a Remove button each (RB-080-30),
// below the field "From the Chrome Web Store" in Engines > Chromium/CEF (`WebStoreInstall.vue`
// mounts it). The rows arrive as the event `webStoreExtensions`, after the request and after
// every change of the store (`installed.ts`). Remove runs the person's command
// `extensions.remove`; the core deletes the folder, ends in a notice and sends the list again.
// An extension this start loaded stays loaded until the next start, which its row says.
import { onMounted, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { WebStoreExtension } from '../ipc/protocol';
import { onInstalled, removeInstalled, requestInstalled } from './installed';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();

const items = ref<WebStoreExtension[]>([]);
const loaded = ref(false);
const busy = ref<string | null>(null);

const stop = onInstalled(props.client, (listed) => {
  items.value = listed;
  loaded.value = true;
});
onScopeDispose(stop);

async function remove(id: string): Promise<void> {
  if (busy.value !== null) {
    return;
  }
  busy.value = id;
  try {
    await removeInstalled(props.client, id);
  } catch {
    // The core's notice names the failure.
  } finally {
    busy.value = null;
  }
}

onMounted(() => {
  requestInstalled(props.client).catch(() => {
    loaded.value = true;
  });
});
</script>

<template>
  <div class="flex flex-col gap-1" data-testid="installed-extensions">
    <p class="text-sm font-medium text-default">{{ t('extensions.installed.title') }}</p>
    <p v-if="loaded && items.length === 0" class="text-xs text-muted" data-testid="installed-extensions-empty">
      {{ t('extensions.installed.empty') }}
    </p>
    <ul v-else class="flex flex-col gap-1">
      <li
        v-for="item in items"
        :key="item.id"
        class="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm"
        data-testid="installed-extension"
        :data-id="item.id"
      >
        <span class="min-w-0 truncate text-default">{{ item.name }} {{ item.version }}</span>
        <span class="font-mono text-xs text-muted">{{ item.id }}</span>
        <span class="text-xs text-muted">
          {{ item.loaded ? t('extensions.installed.loaded') : t('extensions.installed.next_start') }}
        </span>
        <UButton
          size="xs"
          variant="ghost"
          color="error"
          :label="t('extensions.installed.remove')"
          :disabled="busy !== null"
          data-testid="installed-extension-remove"
          @click="remove(item.id)"
        />
      </li>
    </ul>
  </div>
</template>
