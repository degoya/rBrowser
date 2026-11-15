<script setup lang="ts">
// The Portal panel (route `portal` of the pages window, RB-080-05): whether the portal may run
// and runs, the pairing QR code with its countdown, the limits of the replay on a device, and
// the paired devices with "Disconnect". The QR code holds a one-time token; it is drawn from
// the core's module rows, never from markup.
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { PortalState } from '../ipc/protocol';
import {
  clock,
  disconnectMessage,
  loadPortal,
  onPortalChanged,
  qrPath,
  secondsLeft,
  showQrMessage,
  toggleMessage,
} from './portal';

const props = defineProps<{ client: IpcClient }>();
const { t } = useI18n();
const state = ref<PortalState | null>(null);
const error = ref<string | null>(null);
const now = ref(Date.now());

/** The status beside a device's name: unfinished, connected or idle. */
function deviceStatus(device: PortalState['devices'][number]): string {
  if (!device.contacted) return t('portal.panel.unfinished');
  return device.connected ? t('portal.panel.connected') : t('portal.panel.idle');
}

async function reload(): Promise<void> {
  try {
    state.value = await loadPortal(props.client);
    error.value = null;
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  }
}

async function run(message: Parameters<IpcClient['request']>[0]): Promise<void> {
  try {
    await props.client.request(message);
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  }
  await reload();
}

const pairing = computed(() => state.value?.pairing ?? null);
const left = computed(() => (pairing.value ? secondsLeft(pairing.value.expiresAtMs, now.value) : 0));
const path = computed(() => (pairing.value ? qrPath(pairing.value.rows) : ''));

let stop: (() => void) | null = null;
let timer: ReturnType<typeof setInterval> | null = null;
onMounted(() => {
  stop = onPortalChanged(props.client, () => void reload());
  timer = setInterval(() => {
    now.value = Date.now();
  }, 1000);
  void reload();
});
onUnmounted(() => {
  stop?.();
  if (timer !== null) clearInterval(timer);
});
</script>

<template>
  <section class="flex flex-col gap-4 p-4" data-testid="portal-page">
    <h1 class="text-lg font-semibold">{{ t('portal.panel.title') }}</h1>
    <p v-if="error" class="text-sm text-error" role="alert">{{ error }}</p>
    <template v-if="state">
      <p v-if="!state.enabled" class="text-sm" data-testid="portal-disabled">
        {{ t('portal.panel.disabled') }}
      </p>
      <template v-else>
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-sm" data-testid="portal-status">
            {{ state.running ? t('portal.panel.running', { address: state.address ?? '' }) : t('portal.panel.stopped') }}
          </span>
          <UButton
            size="sm"
            :color="state.running ? 'neutral' : 'primary'"
            data-testid="portal-toggle"
            @click="run(toggleMessage(!state.running))"
          >
            {{ state.running ? t('portal.panel.stop') : t('portal.panel.start') }}
          </UButton>
          <UButton
            v-if="state.running"
            size="sm"
            variant="outline"
            data-testid="portal-new-code"
            @click="run(showQrMessage())"
          >
            {{ t('portal.panel.new_code') }}
          </UButton>
        </div>
        <p v-if="state.running" class="text-sm text-warning">{{ t('portal.panel.lan_warning') }}</p>
        <figure v-if="pairing && left > 0" class="flex flex-col items-start gap-2" data-testid="portal-qr">
          <svg
            :viewBox="`-4 -4 ${pairing.size + 8} ${pairing.size + 8}`"
            class="h-56 w-56 bg-white"
            shape-rendering="crispEdges"
            role="img"
            :aria-label="t('portal.panel.qr_label')"
          >
            <path :d="path" fill="#000" />
          </svg>
          <figcaption class="text-sm">
            {{ t('portal.panel.qr_caption', { time: clock(left) }) }}
          </figcaption>
        </figure>
        <p v-else-if="state.running" class="text-sm" data-testid="portal-no-code">
          {{ t('portal.panel.no_code') }}
        </p>
        <p class="text-sm text-muted">{{ t('portal.panel.replay_limits') }}</p>
        <h2 class="font-medium">{{ t('portal.panel.devices') }}</h2>
        <p v-if="state.devices.length === 0" class="text-sm" data-testid="portal-no-devices">
          {{ t('portal.panel.no_devices') }}
        </p>
        <table v-else class="text-sm" data-testid="portal-devices">
          <thead>
            <tr>
              <th class="pr-4 text-left">{{ t('portal.panel.device') }}</th>
              <th class="pr-4 text-left">{{ t('portal.panel.ip') }}</th>
              <th class="pr-4 text-left">{{ t('portal.panel.since') }}</th>
              <th class="pr-4 text-left">{{ t('portal.panel.last_seen') }}</th>
              <th class="pr-4 text-left">{{ t('portal.panel.replays') }}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <tr v-for="device in state.devices" :key="device.id" :data-testid="`portal-device-${device.id}`">
              <td class="pr-4">
                {{ device.name || t('portal.panel.unknown_device') }}
                <span class="text-muted">
                  {{ deviceStatus(device) }}
                </span>
              </td>
              <td class="pr-4">{{ device.ip }}</td>
              <td class="pr-4">{{ new Date(device.pairedAtMs).toLocaleTimeString() }}</td>
              <td class="pr-4">{{ new Date(device.lastSeenMs).toLocaleTimeString() }}</td>
              <td class="pr-4">
                {{ t('portal.panel.counts', { replayed: device.replayed, failed: device.failed, skipped: device.skipped }) }}
              </td>
              <td>
                <UButton
                  size="xs"
                  color="error"
                  variant="outline"
                  :data-testid="`portal-disconnect-${device.id}`"
                  @click="run(disconnectMessage(device.id))"
                >
                  {{ t('portal.panel.disconnect') }}
                </UButton>
              </td>
            </tr>
          </tbody>
        </table>
      </template>
    </template>
  </section>
</template>
