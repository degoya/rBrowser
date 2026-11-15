<script setup lang="ts">
// "Save diagnostics..." (RB-080-73), in Settings > Advanced > Diagnostics and on About: runs
// `diagnostics.create` as the palette and the shortcut do - the core asks where with the Save
// dialog and writes the ZIP there - and waits for its `diagnosticsSaved`: the button is busy and a
// line says the dialog is open, then the path with "Show in folder" (the core opens only the ZIP
// it saved last). A cancelled dialog leaves nothing; a failure points at the status bar, which
// names the reason.
import { onBeforeUnmount, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import { SETTINGS_ICONS } from './icons';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();

type State = { kind: 'idle' } | { kind: 'asking' } | { kind: 'saved'; path: string } | { kind: 'failed' };
const state = ref<State>({ kind: 'idle' });

const stop = props.client.on((message) => {
  if (message.type !== 'diagnosticsSaved' || state.value.kind !== 'asking') {
    return;
  }
  if (message.path !== undefined) {
    state.value = { kind: 'saved', path: message.path };
  } else {
    state.value = message.failed ? { kind: 'failed' } : { kind: 'idle' };
  }
});
onBeforeUnmount(stop);

async function save(): Promise<void> {
  state.value = { kind: 'asking' };
  try {
    await props.client.request({ type: 'runCommand', id: 'diagnostics.create' });
  } catch {
    state.value = { kind: 'failed' };
  }
}

function reveal(): void {
  props.client.request({ type: 'revealDiagnostics' }).catch(() => undefined);
}
</script>

<template>
  <div class="flex flex-col gap-2 px-2 py-3" data-testid="diagnostics">
    <div class="flex flex-wrap items-center gap-3">
      <UButton
        :icon="SETTINGS_ICONS.export"
        color="primary"
        variant="soft"
        size="sm"
        :label="t('settings.diagnostics.save')"
        :loading="state.kind === 'asking'"
        :disabled="state.kind === 'asking'"
        data-testid="diagnostics-save"
        @click="save"
      />
      <p class="min-w-0 flex-1 basis-64 text-xs text-muted">{{ t('settings.diagnostics.what') }}</p>
    </div>
    <div v-if="state.kind !== 'idle'" class="flex flex-wrap items-center gap-2 text-sm" role="status">
      <span class="min-w-0 break-all" :class="state.kind === 'failed' ? 'text-error' : ''" data-testid="diagnostics-status">
        <template v-if="state.kind === 'asking'">{{ t('settings.diagnostics.asking') }}</template>
        <template v-else-if="state.kind === 'saved'">{{ t('settings.diagnostics.saved', { path: state.path }) }}</template>
        <template v-else>{{ t('settings.diagnostics.failed') }}</template>
      </span>
      <UButton
        v-if="state.kind === 'saved'"
        :icon="SETTINGS_ICONS.browseFolder"
        color="neutral"
        variant="ghost"
        size="xs"
        :label="t('settings.diagnostics.show')"
        data-testid="diagnostics-reveal"
        @click="reveal"
      />
    </div>
  </div>
</template>
