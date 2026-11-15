<script setup lang="ts">
// The conflict dialog of the shortcut editor (RB-060-11): a recorded binding another command
// holds. Replace takes it from that command, Swap hands that command this one's first binding,
// Cancel keeps both. Inline in the command's row; the focus moves to "Replace" when it opens and
// Escape cancels.
import { onMounted, ref, useId } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { ConflictChoice } from './conflict';

const props = defineProps<{ binding: string; holder: string; old?: string }>();
const emit = defineEmits<{ choose: [choice: ConflictChoice] }>();
const { t } = useI18n();
const textId = useId();
const root = ref<HTMLElement | null>(null);

onMounted(() => root.value?.querySelector<HTMLElement>('[data-testid="conflict-replace"]')?.focus());

function onKey(event: KeyboardEvent): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    emit('choose', 'cancel');
  }
}
</script>

<template>
  <div
    ref="root"
    role="alertdialog"
    :aria-labelledby="textId"
    class="flex basis-full flex-wrap items-center gap-2 rounded-md bg-warning/10 px-2 py-1.5"
    data-testid="conflict"
    @keydown="onKey"
  >
    <p :id="textId" class="text-xs text-default">
      {{ t('settings.shortcuts.conflict', { binding: props.binding, command: props.holder }) }}
    </p>
    <UButton size="xs" color="warning" variant="soft" :label="t('settings.shortcuts.conflict_replace')" data-testid="conflict-replace" @click="emit('choose', 'replace')" />
    <UButton
      v-if="props.old !== undefined"
      size="xs"
      color="neutral"
      variant="soft"
      :label="t('settings.shortcuts.conflict_swap', { binding: props.old })"
      data-testid="conflict-swap"
      @click="emit('choose', 'swap')"
    />
    <UButton size="xs" color="neutral" variant="ghost" :label="t('settings.shortcuts.conflict_cancel')" data-testid="conflict-cancel" @click="emit('choose', 'cancel')" />
  </div>
</template>
