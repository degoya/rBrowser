import { describe, expect, it } from 'vitest';
import builtin from '../../../crates/rb-config/data/devices.json';
import type { DevicePreset } from '../ipc/protocol';
import { createShellI18n } from '../i18n';
import { deviceName, deviceSize, groupByKind } from './devices';

const devices = builtin as DevicePreset[];
const { t } = createShellI18n().global;

describe('device presets in the shell', () => {
  it('names every built-in device through the catalogue', () => {
    for (const device of devices) {
      const name = deviceName(device, t);
      expect(name, device.id).not.toBe(`devices.${device.id}`);
      expect(name).not.toBe('');
    }
    expect(deviceName(devices.find((d) => d.id === 'iphone-15')!, t)).toBe('iPhone 15');
  });

  it('prefers the label of a user preset', () => {
    const own: DevicePreset = { ...devices[0]!, id: 'mine', label: 'My phone' };
    expect(deviceName(own, t)).toBe('My phone');
  });

  it('shows the viewport and pixel ratio', () => {
    expect(deviceSize(devices.find((d) => d.id === 'iphone-15')!, t)).toBe('393 x 852 at 3x');
  });

  it('groups by kind in the order phones, tablets, desktops and drops empty groups', () => {
    const groups = groupByKind(devices);
    expect(groups.map(([kind]) => kind)).toEqual(['mobile', 'tablet', 'desktop']);
    expect(groups.flatMap(([, list]) => list)).toHaveLength(devices.length);
    expect(groupByKind(devices.filter((d) => d.kind === 'tablet')).map(([k]) => k)).toEqual([
      'tablet',
    ]);
  });
});
