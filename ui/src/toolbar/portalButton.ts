// The portal's button in the toolbar (RB-090-62, `PortalButton.vue`): pairing a real phone or
// tablet in the LAN (RB-080-05) one click away instead of only behind `Leader S P`. Three states -
// off, on without a device, on with the number of paired devices on its chip - each named in
// words. Off, a click runs `portal.toggle` on, which starts the portal (only where a person
// allowed it with `portal.enabled`, a LAN listener being a human's decision) and shows a fresh
// pairing QR code. On, a click opens the core's native menu below it (RB-090-45, `nativeMenu.ts`)
// with the QR code, the devices, a device to disconnect and the stop - the portal's existing
// commands. The state comes from `getPortal`, asked again on every `portalChanged`. Where a person
// allowed the presentation over the internet (RB-096-05), the menu starts or ends it too.

import type { InjectionKey } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { NativeMenuRow, PortalState, ShellToCore } from '../ipc/protocol';
import { ICONS } from '../icons';
import {
  DISCONNECT_COMMAND,
  disconnectMessage,
  loadPortal,
  showQrMessage,
  toggleMessage,
} from '../portal/portal';
import { isPresenting, startMessage as presentMessage, stopMessage as endPresentMessage } from '../portal/presentation';
import type { Subscribe } from '../whichkey/whichkey';

/** The button's icon: a phone and a tablet. */
export const PORTAL_ICON = ICONS.portal;
/** `Leader P D`: the Portal panel with the paired devices. */
export const PORTAL_DEVICES_COMMAND = 'portal.devices';
/** Whether the toolbar shows the button. */
export const PORTAL_BUTTON_SETTING = 'ui.toolbar.portalButton';

/** Where the button reads the portal's state: `getPortal`, and the core's events. */
export interface PortalSource {
  load: () => Promise<PortalState>;
  subscribe: Subscribe;
}

/** The shell's portal source (`bootstrap.ts` provides it). */
export const PORTAL_SOURCE_KEY: InjectionKey<PortalSource> = Symbol('rb-portal-source');

/** The source over the shell's IPC client. */
export function portalSource(client: IpcClient): PortalSource {
  return { load: () => loadPortal(client), subscribe: (listener) => client.on(listener) };
}

/** `t` of vue-i18n, as far as the button needs it. */
export type Translate = (key: string, args?: Record<string, unknown>) => string;

/** Off; on without a device; on with paired devices. */
export type PortalLook = 'off' | 'idle' | 'paired';

/** The state of the button for `state` (`null`: not known yet, or the core could not say). */
export function portalLook(state: PortalState | null): PortalLook {
  if (state === null || !state.running) {
    return 'off';
  }
  return state.devices.length === 0 ? 'idle' : 'paired';
}

/** The button's name: the state in words, the number of devices included. */
export function portalLabel(state: PortalState | null, t: Translate): string {
  const look = portalLook(state);
  if (look !== 'paired') {
    return t(`shell.toolbar.portal.${look}`);
  }
  return t('shell.toolbar.portal.paired', { count: state?.devices.length ?? 0 });
}

/** The number on the chip, at most two characters. */
export function chipText(count: number): string {
  return count > 9 ? '9+' : String(count);
}

const DEVICE = 'device:';
const ALL = `${DEVICE}all`;

/** The menu while the portal runs; none while it is off (the click starts it). */
export function portalRows(state: PortalState | null, t: Translate): NativeMenuRow[] {
  if (state === null || portalLook(state) === 'off') {
    return [];
  }
  const devices: NativeMenuRow[] =
    state.devices.length === 0
      ? [{ kind: 'item', id: `${DEVICE}none`, label: t('portal.panel.no_devices'), disabled: true }]
      : state.devices.map((device) => ({
          kind: 'item',
          id: `${DEVICE}${device.id}`,
          label: t('shell.toolbar.portal.menu.device', {
            name: device.name === '' ? t('portal.panel.unknown_device') : device.name,
            ip: device.ip,
          }),
        }));
  if (state.devices.length > 1) {
    devices.push({ kind: 'item', id: ALL, label: t('shell.toolbar.portal.menu.disconnect_all') });
  }
  const presentation = state.presentation;
  const present: NativeMenuRow[] =
    presentation?.enabled === true
      ? [
          { kind: 'separator' },
          isPresenting(presentation)
            ? { kind: 'item', id: 'present_stop', label: t('shell.toolbar.portal.menu.present_stop') }
            : { kind: 'item', id: 'present', label: t('shell.toolbar.portal.menu.present') },
        ]
      : [];
  return [
    { kind: 'item', id: 'show_qr', label: t('shell.toolbar.portal.menu.show_qr') },
    { kind: 'item', id: 'devices', label: t('shell.toolbar.portal.menu.devices') },
    { kind: 'separator' },
    { kind: 'heading', label: t('shell.toolbar.portal.menu.disconnect') },
    ...devices,
    ...present,
    { kind: 'separator' },
    { kind: 'item', id: 'stop', label: t('portal.panel.stop') },
  ];
}

/** What the row `id` runs; `null` for a row that runs nothing. */
export function portalChoice(id: string): ShellToCore | null {
  switch (id) {
    case 'show_qr':
      return showQrMessage();
    case 'devices':
      return { type: 'runCommand', id: PORTAL_DEVICES_COMMAND };
    case 'stop':
      return toggleMessage(false);
    case 'present':
      return presentMessage({ consent: false, download: false });
    case 'present_stop':
      return endPresentMessage();
    case ALL:
      return { type: 'runCommand', id: DISCONNECT_COMMAND };
    default: {
      const device = id.startsWith(DEVICE) ? id.slice(DEVICE.length) : '';
      return /^\d+$/.test(device) ? disconnectMessage(Number(device)) : null;
    }
  }
}

/** The click while the portal is off: start it with a fresh pairing QR code. */
export function startMessage(): ShellToCore {
  return toggleMessage(true);
}
