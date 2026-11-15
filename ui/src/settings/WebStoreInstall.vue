<script setup lang="ts">
// "From the Chrome Web Store" (RB-080-23): an id or a store URL and Install, below the switch
// `engines.chromium.extensions.allowWebStore` in Engines > Chromium/CEF. It runs the person's
// command `extensions.install_web_store`; the download, the CRX3 check and the unpacking follow
// in the core and end in a notice. Nothing is sent for a reference that is no id or store URL.
// Below the field, the installed Web Store extensions with a Remove button each (RB-080-30).
import { computed, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import InstalledExtensions from '../extensions/InstalledExtensions.vue';
import type { IpcClient } from '../ipc/client';
import { INSTALL_COMMAND, parseStoreRef } from './webStoreRef';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const id = useId();

const written = ref('');
const invalid = ref(false);
const busy = ref(false);
const started = ref(false);
const hint = computed(() => {
  if (invalid.value) {
    return t('extensions.web_store.invalid');
  }
  return started.value ? t('extensions.web_store.started') : t('extensions.web_store.hint');
});

async function install(): Promise<void> {
  const reference = parseStoreRef(written.value);
  if (reference === null) {
    invalid.value = true;
    return;
  }
  invalid.value = false;
  busy.value = true;
  try {
    await props.client.request({ type: 'runCommand', id: INSTALL_COMMAND, args: { ref: written.value.trim() } });
    started.value = true;
    written.value = '';
  } catch {
    started.value = false;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col gap-3">
  <form class="flex flex-col gap-1" data-testid="web-store-install" @submit.prevent="install">
    <label :for="`${id}-ref`" class="text-sm font-medium text-default">{{ t('extensions.web_store.label') }}</label>
    <div class="flex flex-wrap items-center gap-2">
      <input
        :id="`${id}-ref`"
        v-model="written"
        type="text"
        autocomplete="off"
        spellcheck="false"
        :placeholder="t('extensions.web_store.placeholder')"
        class="min-w-0 flex-1 basis-72 rounded-sm border bg-default px-2 py-1 text-sm focus-visible:outline-2 focus-visible:outline-primary"
        :class="invalid ? 'border-error' : 'border-default'"
        :aria-invalid="invalid ? 'true' : undefined"
        :aria-describedby="`${id}-hint`"
        data-testid="web-store-ref"
        @input="invalid = false; started = false"
      />
      <UButton type="submit" size="xs" :label="t('extensions.web_store.install')" :disabled="busy" data-testid="web-store-install-button" />
    </div>
    <p :id="`${id}-hint`" class="text-xs" :class="invalid ? 'text-error' : 'text-muted'" data-testid="web-store-hint">
      {{ hint }}
    </p>
  </form>
  <InstalledExtensions :client="client" />
  </div>
</template>
