<script setup lang="ts">
// The emulation extras of the toolbar (RB-080-10), in a popover beside the colour scheme and
// reduced motion: geolocation (presets and free input), time zone, CPU throttling, offline and
// a print view. Every choice is a `view.emulate.*` command for the active view; Shift-click
// applies it to every view. The pressed state is the active view's emulation.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UPopover from '@nuxt/ui/components/Popover.vue';
import type { Emulation, JsonValue } from '../ipc/protocol';
import { useShell } from '../state/shell';
import {
  CPU_RATES,
  EXTRA_COMMANDS,
  GEOLOCATION_PRESETS,
  PAPERS,
  PRESET_LABEL_KEYS,
  TIMEZONES,
  addPrintMessage,
  extraMessage,
  isOffline,
  parseCoordinate,
  presetOf,
  type Paper,
} from './extras';

const props = defineProps<{ current: Emulation | null; disabled: boolean }>();
const emit = defineEmits<{ changed: [] }>();

const shell = useShell();
const { t } = useI18n();

const latitude = ref('');
const longitude = ref('');
const timezone = ref('');
const position = computed(() => parseCoordinatePair(latitude.value, longitude.value));
const preset = computed(() => presetOf(props.current));
const offline = computed(() => isOffline(props.current));

function parseCoordinatePair(lat: string, lon: string): { latitude: number; longitude: number } | null {
  const latValue = parseCoordinate(lat, -90, 90);
  const lonValue = parseCoordinate(lon, -180, 180);
  return latValue === null || lonValue === null ? null : { latitude: latValue, longitude: lonValue };
}

async function run(
  id: (typeof EXTRA_COMMANDS)[keyof typeof EXTRA_COMMANDS],
  args: { [key: string]: JsonValue },
  event: MouseEvent,
): Promise<void> {
  if (await shell.send(extraMessage(id, args, event.shiftKey))) {
    emit('changed');
  }
}

function setPreset(id: string, event: MouseEvent): void {
  void run(EXTRA_COMMANDS.geolocation, { preset: id }, event);
}

function setPosition(event: MouseEvent): void {
  if (position.value !== null) {
    void run(EXTRA_COMMANDS.geolocation, { geolocation: position.value }, event);
  }
}

function setTimezone(value: string, event: MouseEvent): void {
  void run(EXTRA_COMMANDS.timezone, { timezone: value }, event);
}

function setRate(rate: number, event: MouseEvent): void {
  void run(EXTRA_COMMANDS.cpu, { rate }, event);
}

function toggleOffline(event: MouseEvent): void {
  void run(EXTRA_COMMANDS.offline, { offline: !offline.value }, event);
}

async function addPrint(paper: Paper): Promise<void> {
  if (await shell.send(addPrintMessage(paper))) {
    emit('changed');
  }
}
</script>

<template>
  <UPopover :portal="false">
    <UButton
      icon="i-lucide-sliders-horizontal"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :disabled="disabled"
      :aria-label="t('emulation.extras.open')"
      :title="`${t('emulation.extras.open')} - ${t('emulation.toolbar.allHint')}`"
      data-testid="emulation-extras"
    />
    <template #content>
      <div class="flex w-72 flex-col gap-3 p-3 text-sm" data-testid="emulation-extras-panel">
        <p class="text-xs text-muted">{{ t('emulation.toolbar.allHint') }}</p>
        <section :aria-label="t('emulation.extras.geolocation')" class="flex flex-col gap-1">
          <h3 class="font-medium">{{ t('emulation.extras.geolocation') }}</h3>
          <div class="flex flex-wrap gap-1">
            <UButton
              v-for="item in GEOLOCATION_PRESETS"
              :key="item.id"
              size="xs"
              color="neutral"
              :variant="preset === item.id ? 'soft' : 'ghost'"
              :aria-pressed="preset === item.id"
              :data-testid="`geo-${item.id}`"
              :label="t(PRESET_LABEL_KEYS[item.id] ?? item.id)"
              @click="setPreset(item.id, $event)"
            />
            <UButton
              size="xs"
              color="neutral"
              variant="ghost"
              data-testid="geo-off"
              :label="t('emulation.extras.off')"
              @click="run(EXTRA_COMMANDS.geolocation, { geolocation: null }, $event)"
            />
          </div>
          <div class="flex gap-1">
            <UInput
              v-model="latitude"
              size="xs"
              :aria-label="t('emulation.extras.latitude')"
              :placeholder="t('emulation.extras.latitude')"
              data-testid="geo-latitude"
            />
            <UInput
              v-model="longitude"
              size="xs"
              :aria-label="t('emulation.extras.longitude')"
              :placeholder="t('emulation.extras.longitude')"
              data-testid="geo-longitude"
            />
            <UButton
              size="xs"
              :disabled="position === null"
              data-testid="geo-apply"
              :label="t('emulation.extras.apply')"
              @click="setPosition($event)"
            />
          </div>
        </section>
        <section :aria-label="t('emulation.extras.timezone')" class="flex flex-col gap-1">
          <h3 class="font-medium">{{ t('emulation.extras.timezone') }}</h3>
          <div class="flex gap-1">
            <UInput
              v-model="timezone"
              size="xs"
              list="rb-timezones"
              :aria-label="t('emulation.extras.timezone')"
              :placeholder="current?.timezone ?? 'America/New_York'"
              data-testid="tz-input"
            />
            <datalist id="rb-timezones">
              <option v-for="zone in TIMEZONES" :key="zone" :value="zone" />
            </datalist>
            <UButton
              size="xs"
              :disabled="timezone.trim() === ''"
              data-testid="tz-apply"
              :label="t('emulation.extras.apply')"
              @click="setTimezone(timezone.trim(), $event)"
            />
            <UButton
              size="xs"
              color="neutral"
              variant="ghost"
              data-testid="tz-off"
              :label="t('emulation.extras.off')"
              @click="setTimezone('', $event)"
            />
          </div>
        </section>
        <section :aria-label="t('emulation.extras.cpu')" class="flex flex-col gap-1">
          <h3 class="font-medium">{{ t('emulation.extras.cpu') }}</h3>
          <div class="flex flex-wrap gap-1">
            <UButton
              size="xs"
              color="neutral"
              :variant="current?.cpuThrottling === undefined ? 'soft' : 'ghost'"
              data-testid="cpu-off"
              :label="t('emulation.extras.off')"
              @click="setRate(1, $event)"
            />
            <UButton
              v-for="rate in CPU_RATES"
              :key="rate"
              size="xs"
              color="neutral"
              :variant="current?.cpuThrottling === rate ? 'soft' : 'ghost'"
              :aria-pressed="current?.cpuThrottling === rate"
              :data-testid="`cpu-${rate}`"
              :label="t('emulation.extras.cpuRate', { rate })"
              @click="setRate(rate, $event)"
            />
          </div>
        </section>
        <section class="flex flex-wrap items-center gap-1">
          <UButton
            size="xs"
            color="neutral"
            icon="i-lucide-wifi-off"
            :variant="offline ? 'soft' : 'ghost'"
            :aria-pressed="offline"
            data-testid="offline"
            :label="t('emulation.extras.offline')"
            @click="toggleOffline($event)"
          />
          <UButton
            v-for="paper in PAPERS"
            :key="paper"
            size="xs"
            color="neutral"
            variant="ghost"
            icon="i-lucide-printer"
            :data-testid="`print-${paper}`"
            :label="t('emulation.extras.print', { paper })"
            @click="addPrint(paper)"
          />
        </section>
      </div>
    </template>
  </UPopover>
</template>
