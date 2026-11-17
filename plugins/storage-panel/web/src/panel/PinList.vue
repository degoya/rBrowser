<script setup lang="ts">
// The pinned values (RB-070-01, on Nuxt UI since RB-080-84): each with a switch that turns it on
// or off, what it sets where, its session, and a remove button; a note when there are none, when
// pins are switched off in the settings or the workspace is not trusted (the file is not read).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import type { StoragePin } from './model';

const props = defineProps<{ pins: StoragePin[]; untrusted: boolean }>();
const emit = defineEmits<{ enable: [id: string, enabled: boolean]; remove: [id: string] }>();
const { t, settings } = usePanel();

const note = computed(() => {
  if (props.untrusted) return t('panel.pinsUntrusted');
  if (settings.value['storage-panel.pinsEnabled'] === false) return t('panel.pinsOff');
  return props.pins.length === 0 ? t('panel.pinsEmpty') : '';
});

function text(pin: StoragePin): string {
  const parts = [pin.kind, pin.origin, pin.key + ' = ' + pin.value + (pin.truncated ? '…' : '')];
  if (pin.session) parts.push(t('panel.session', { name: pin.session }));
  return parts.join(' · ');
}
</script>

<template>
  <section class="flex flex-col gap-1.5" aria-labelledby="pins-title">
    <h2 id="pins-title" class="flex items-center gap-1.5 text-xs font-semibold uppercase text-toned">
      <UIcon name="i-lucide-pin" class="size-4" />
      <span>{{ t('panel.pins') }}</span>
    </h2>
    <p v-if="note" id="pins-note" class="text-sm text-muted">{{ note }}</p>
    <ul id="pins" class="divide-y divide-default">
      <li v-for="pin in pins" :key="pin.id" data-pin class="flex items-center gap-2 py-1.5">
        <USwitch
          :model-value="pin.enabled"
          :data-id="pin.id"
          size="sm"
          :aria-label="t('panel.enabled') + ': ' + pin.key"
          @update:model-value="(on: boolean) => emit('enable', pin.id, on)"
        />
        <span data-text class="min-w-0 flex-1 break-all font-mono text-xs">{{ text(pin) }}</span>
        <IconAction :data-id="pin.id" icon="i-lucide-trash-2" :label="t('panel.remove') + ': ' + pin.key" @click="emit('remove', pin.id)" />
      </li>
    </ul>
  </section>
</template>
