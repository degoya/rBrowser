<script setup lang="ts">
// The device presets in effect, grouped by kind (Settings > Breakpoints & Devices > Device
// presets, and the choice when a breakpoint is created; RB-010-08 mounts it with the answer to
// `getDevices`). Read-only: the user's own presets are the setting `devices.items`. Each preset is
// a Nuxt UI `UButton` (ghost, block; RB-080-88): the name, and the size at the end.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { DevicePreset } from '../ipc/protocol';
import { deviceName, deviceSize, groupByKind } from './devices';

const props = defineProps<{ items: DevicePreset[] }>();
const emit = defineEmits<{ pick: [id: string] }>();
const { t } = useI18n();
const groups = computed(() => groupByKind(props.items));
</script>

<template>
  <section :aria-label="t('presets.devices.label')" class="flex flex-col gap-3 text-sm">
    <p v-if="groups.length === 0" class="text-muted">{{ t('presets.devices.empty') }}</p>
    <div v-for="[kind, devices] in groups" :key="kind" class="flex flex-col gap-1">
      <h3 class="font-medium text-highlighted">{{ t(`presets.devices.kind.${kind}`) }}</h3>
      <ul class="flex flex-col">
        <li v-for="device in devices" :key="device.id">
          <UButton
            color="neutral"
            variant="ghost"
            block
            class="justify-between gap-2 text-start font-normal"
            :data-device="device.id"
            @click="emit('pick', device.id)"
          >
            <span class="text-default">{{ deviceName(device, t) }}</span>
            <span class="text-muted">
              {{ deviceSize(device, t) }}
              <template v-if="device.touch"> - {{ t('presets.devices.touch') }}</template>
            </span>
          </UButton>
        </li>
      </ul>
    </div>
  </section>
</template>
