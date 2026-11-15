// The device presets as the shell shows them (RB-010-12): the answer to `getDevices`, grouped by
// kind; a built-in preset is named by the i18n key `devices.<id>`, the user's by its `label`.

import type { DevicePreset } from '../ipc/protocol';

export type DeviceKind = DevicePreset['kind'];

export const DEVICE_KINDS: readonly DeviceKind[] = ['mobile', 'tablet', 'desktop'];

type Translate = (key: string, args?: Record<string, string | number>) => string;

/** The name of `device`: its own label, else the catalogue's `devices.<id>`. */
export function deviceName(device: DevicePreset, t: Translate): string {
  return device.label ?? t(`devices.${device.id}`);
}

/** `393 x 852 at 3x`. */
export function deviceSize(device: DevicePreset, t: Translate): string {
  return t('presets.devices.size', {
    width: device.viewport.width,
    height: device.viewport.height,
    dpr: device.dpr,
  });
}

/** The presets per kind, each group in list order; empty groups are left out. */
export function groupByKind(items: readonly DevicePreset[]): [DeviceKind, DevicePreset[]][] {
  return DEVICE_KINDS.map((kind): [DeviceKind, DevicePreset[]] => [
    kind,
    items.filter((device) => device.kind === kind),
  ]).filter(([, devices]) => devices.length > 0);
}
