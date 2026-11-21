<script setup lang="ts">
// The Portal panel (route `portal` of the pages window, RB-080-05): whether the portal may run
// and runs, the pairing QR code with its countdown, the limits of the replay on a device, and
// the paired devices with "Disconnect". The QR code holds a one-time token; it is drawn from
// the core's module rows, never from markup. The devices are a Nuxt UI `UTable` (RB-080-88).
// Below, the presentation over the internet (RB-096-05, `PresentationSection.vue`), shown
// whether the LAN portal may run or not. Off, the hint opens Settings at `portal.enabled`
// (RB-094-21).
import { computed, h, onMounted, onUnmounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { TableColumn } from '@nuxt/ui';
import UButton from '@nuxt/ui/components/Button.vue';
import UTable from '@nuxt/ui/components/Table.vue';
import type { IpcClient } from '../ipc/client';
import type { PortalState } from '../ipc/protocol';
import SettingLink from '../settings/SettingLink.vue';
import PresentationSection from './PresentationSection.vue';
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

type Device = PortalState['devices'][number];

const columns = computed<TableColumn<Device>[]>(() => [
  { id: 'device', header: t('portal.panel.device') },
  { accessorKey: 'ip', header: t('portal.panel.ip') },
  { id: 'since', header: t('portal.panel.since'), cell: ({ row }) => new Date(row.original.pairedAtMs).toLocaleTimeString() },
  {
    id: 'lastSeen',
    header: t('portal.panel.last_seen'),
    cell: ({ row }) => new Date(row.original.lastSeenMs).toLocaleTimeString(),
  },
  {
    id: 'replays',
    header: t('portal.panel.replays'),
    cell: ({ row }) =>
      t('portal.panel.counts', { replayed: row.original.replayed, failed: row.original.failed, skipped: row.original.skipped }),
  },
  { id: 'actions', header: () => h('span', { class: 'sr-only' }, t('portal.panel.disconnect')) },
]);

/** The status beside a device's name: unfinished, connected or idle. */
function deviceStatus(device: Device): string {
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
      <div v-if="!state.enabled" class="flex flex-col items-start gap-1">
        <p class="text-sm" data-testid="portal-disabled">
          {{ t('portal.panel.disabled') }}
        </p>
        <SettingLink setting-key="portal.enabled" @open="run" />
      </div>
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
        <UTable
          v-else
          :data="state.devices"
          :columns="columns"
          :get-row-id="(device: Device) => String(device.id)"
          :ui="{ th: 'px-2 py-1.5', td: 'px-2 py-1.5' }"
          data-testid="portal-devices"
        >
          <template #device-cell="{ row }">
            <span class="text-default" :data-testid="`portal-device-${row.original.id}`">
              {{ row.original.name || t('portal.panel.unknown_device') }}
              <span class="text-muted">{{ deviceStatus(row.original) }}</span>
            </span>
          </template>
          <template #actions-cell="{ row }">
            <UButton
              size="xs"
              color="error"
              variant="outline"
              :data-testid="`portal-disconnect-${row.original.id}`"
              @click="run(disconnectMessage(row.original.id))"
            >
              {{ t('portal.panel.disconnect') }}
            </UButton>
          </template>
        </UTable>
      </template>
      <PresentationSection
        v-if="state.presentation"
        :presentation="state.presentation"
        :now-ms="now"
        @run="run"
      />
    </template>
  </section>
</template>
