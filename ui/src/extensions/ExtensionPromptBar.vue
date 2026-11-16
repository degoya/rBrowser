<script setup lang="ts">
// The question before a Chrome Web Store extension is installed on an MCP client's request or
// updated (RB-080-23), or removed on an MCP client's request (RB-080-30): the core's
// `extensionPrompt` names the verified name and version, this bar covers the toolbar strip with
// it, and the person's `extensionDecision` is the only way the install or removal goes on.
// Escape and "Not now" decline. Prompts queue: the update check asks once per
// extension, each is shown after the one before it is answered; a second prompt for the same
// extension takes the earlier one's place, as the core forgets that one. Mounted in `App.vue`.
// The answers are `UButton`s (RB-080-88).
import { computed, onScopeDispose, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { ExtensionPrompt } from '../ipc/protocol';
import { useShell } from '../state/shell';
import { windowEvents, type Subscribe } from '../whichkey/whichkey';

const props = withDefaults(defineProps<{ subscribe?: Subscribe }>(), { subscribe: undefined });
const shell = useShell();
const { t } = useI18n();

const queue = ref<ExtensionPrompt[]>([]);
const prompt = computed<ExtensionPrompt | null>(() => queue.value[0] ?? null);
const busy = ref(false);

const text = computed(() => {
  const open = prompt.value;
  if (open === null) {
    return '';
  }
  if (open.kind === 'update') {
    return t('extensions.prompt.update', { name: open.name, version: open.version, current: open.currentVersion ?? '' });
  }
  if (open.kind === 'remove') {
    return t('extensions.prompt.remove', { name: open.name, version: open.version });
  }
  return t('extensions.prompt.install', { name: open.name, version: open.version });
});
const ACCEPT_KEYS = {
  install: 'extensions.prompt.install_button',
  update: 'extensions.prompt.update_button',
  remove: 'extensions.prompt.remove_button',
} as const;
const acceptLabel = computed(() => t(ACCEPT_KEYS[prompt.value?.kind ?? 'install']));

const stop = (props.subscribe ?? windowEvents)((message) => {
  if (message.type === 'extensionPrompt') {
    const { type: _type, ...next } = message;
    const earlier = queue.value.findIndex((open) => open.id === next.id);
    if (earlier === -1) {
      queue.value.push(next);
    } else {
      queue.value.splice(earlier, 1, next);
      if (earlier === 0) {
        busy.value = false;
      }
    }
  }
});
onScopeDispose(stop);

async function decide(accepted: boolean): Promise<void> {
  const open = prompt.value;
  if (open === null || busy.value) {
    return;
  }
  busy.value = true;
  const sent = await shell.send({ type: 'extensionDecision', promptId: open.promptId, accepted });
  busy.value = false;
  if (sent || !accepted) {
    queue.value = queue.value.filter((queued) => queued.promptId !== open.promptId);
  }
}
</script>

<template>
  <div
    v-if="prompt !== null"
    role="alertdialog"
    :aria-label="text"
    class="fixed inset-x-0 top-0 z-50 flex h-11 items-center gap-2 overflow-x-auto border-b border-default bg-elevated px-3 text-sm text-default"
    data-testid="extension-prompt"
    tabindex="-1"
    @keydown.escape.prevent="decide(false)"
  >
    <span class="min-w-0 flex-1 truncate" data-testid="extension-prompt-text">{{ text }}</span>
    <UButton size="sm" class="shrink-0" :disabled="busy" data-testid="extension-prompt-accept" @click="decide(true)">
      {{ acceptLabel }}
    </UButton>
    <UButton
      size="sm"
      color="neutral"
      variant="ghost"
      class="shrink-0"
      :disabled="busy"
      data-testid="extension-prompt-decline"
      @click="decide(false)"
    >
      {{ t('extensions.prompt.decline') }}
    </UButton>
  </div>
</template>
