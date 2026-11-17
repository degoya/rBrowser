<script setup lang="ts">
// The portal's button in the toolbar (RB-090-62, `portalButton.ts`): pairs a phone or tablet in
// the LAN (RB-080-05) from the toolbar. Its name and tooltip say the state - off, on without a
// device, on with n devices - and the tooltip names `portal.toggle`'s shortcut; the number of
// devices stands on a chip, the button is tinted while the portal listens. Off, a click runs
// `portal.toggle` on: the core starts the portal only where a person allowed it
// (`portal.enabled`; otherwise the status bar says where to) and shows a fresh pairing QR code.
// On, a click, Enter, Space, an arrow key or F4 opens the core's native menu below it (RB-090-45,
// `nativeMenu.ts`) - QR code, devices, a device to disconnect, stop - and the focus comes back to
// the button. The state is `getPortal`, asked again on every `portalChanged`. In a narrow window
// its group leaves the row: off it is one entry of the overflow menu, on its rows stand there
// under its name (`provideMenuSource`). `ui.toolbar.portalButton` off: nothing is drawn.
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UChip from '@nuxt/ui/components/Chip.vue';
import type { PortalState } from '../ipc/protocol';
import { TOGGLE_COMMAND } from '../portal/portal';
import { booleanSetting } from '../settings/booleanSetting';
import { useShell } from '../state/shell';
import { isOpeningKey, provideMenuSource, useNativeMenu } from './nativeMenu';
import {
  chipText,
  PORTAL_BUTTON_SETTING,
  PORTAL_ICON,
  PORTAL_SOURCE_KEY,
  portalChoice,
  portalLabel,
  portalLook,
  portalRows,
  startMessage,
  type PortalSource,
} from './portalButton';

const props = withDefaults(defineProps<{ source?: PortalSource }>(), { source: undefined });
const shell = useShell();
const { t } = useI18n();
const menu = useNativeMenu('portal', { send: shell.send });
const source = props.source ?? inject(PORTAL_SOURCE_KEY, null);

const shown = computed(() => booleanSetting(shell.settings.value[PORTAL_BUTTON_SETTING], true));
const portal = ref<PortalState | null>(null);
/** Only the newest answer counts: a slow one must not overwrite a later state. */
let asked = 0;
async function reload(): Promise<void> {
  if (source === null) {
    return;
  }
  asked += 1;
  const mine = asked;
  try {
    const state = await source.load();
    if (mine === asked) {
      portal.value = state;
    }
  } catch {
    // The core could not say: the button shows off, and a click asks the core again.
    if (mine === asked) {
      portal.value = null;
    }
  }
}
const stopEvents = source?.subscribe((message) => {
  if (message.type === 'portalChanged') {
    void reload();
  }
});
onBeforeUnmount(() => stopEvents?.());
onMounted(() => {
  shell.loadCommands();
  void reload();
});

const look = computed(() => portalLook(portal.value));
const running = computed(() => look.value !== 'off');
const count = computed(() => portal.value?.devices.length ?? 0);
const label = computed(() => portalLabel(portal.value, t));
const title = computed(() => {
  const shortcut = shell.bindingOf(TOGGLE_COMMAND);
  return shortcut === null ? label.value : t('shell.toolbar.portal.shortcut', { label: label.value, shortcut });
});

const anchor = ref<HTMLElement | null>(null);

function rows() {
  return portalRows(portal.value, t);
}

function choose(id: string): void {
  const message = portalChoice(id);
  if (message !== null) {
    void shell.send(message);
  }
}

function open(): void {
  if (anchor.value !== null && running.value) {
    void menu.open(anchor.value, rows(), choose);
  }
}

function onClick(): void {
  if (running.value) {
    open();
  } else {
    void shell.send(startMessage());
  }
}

function onKeyDown(event: KeyboardEvent): void {
  if (running.value && isOpeningKey(event)) {
    event.preventDefault();
    open();
  }
}

/** The rows for the overflow menu while the portal runs; off, the button is one entry there. */
let stopSource: (() => void) | null = null;
watch(
  [anchor, running],
  ([element, on]) => {
    stopSource?.();
    const button = element?.querySelector('button');
    stopSource = button && on ? provideMenuSource(button, { rows, choose }) : null;
  },
  { flush: 'post' },
);
onBeforeUnmount(() => stopSource?.());
</script>

<template>
  <span v-if="shown" ref="anchor" class="inline-flex shrink-0 items-center">
    <UChip :show="count > 0" :text="chipText(count)" color="primary" size="3xl" inset :ui="{ base: 'px-0.5' }">
      <UButton
        :icon="PORTAL_ICON"
        :color="running ? 'primary' : 'neutral'"
        :variant="running ? 'soft' : 'ghost'"
        size="sm"
        square
        class="size-8 justify-center"
        :aria-haspopup="running ? 'menu' : undefined"
        :aria-label="label"
        :title="title"
        :data-icon="PORTAL_ICON"
        :data-portal="look"
        data-testid="portal-button"
        @click="onClick"
        @keydown="onKeyDown"
      />
    </UChip>
  </span>
</template>
