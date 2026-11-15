import { describe, expect, it } from 'vitest';
import type { Emulation } from '../ipc/protocol';
import {
  EXTRA_COMMANDS,
  addPrintMessage,
  extraMessage,
  isOffline,
  parseCoordinate,
  presetOf,
} from './extras';

const base: Emulation = { width: 800, height: 600, dpr: 1, mobile: false, touch: false, media: {} };

describe('the emulation extras', () => {
  it('target the active view, or every view with Shift', () => {
    expect(extraMessage(EXTRA_COMMANDS.cpu, { rate: 4 }, false)).toEqual({
      type: 'runCommand',
      id: 'view.emulate.cpu',
      args: { rate: 4 },
    });
    expect(extraMessage(EXTRA_COMMANDS.geolocation, { preset: 'tokyo' }, true)).toEqual({
      type: 'runCommand',
      id: 'view.emulate.geolocation',
      args: { preset: 'tokyo', target: 'all' },
    });
    expect(extraMessage(EXTRA_COMMANDS.geolocation, { geolocation: null }, false)).toEqual({
      type: 'runCommand',
      id: 'view.emulate.geolocation',
      args: { geolocation: null },
    });
  });

  it('open a print view on the chosen paper', () => {
    expect(addPrintMessage('Letter')).toEqual({
      type: 'runCommand',
      id: 'view.add_print',
      args: { paper: 'Letter' },
    });
  });

  it('read typed coordinates', () => {
    expect(parseCoordinate(' 52.52 ', -90, 90)).toBe(52.52);
    expect(parseCoordinate('13,4', -180, 180)).toBe(13.4);
    expect(parseCoordinate('91', -90, 90)).toBeNull();
    expect(parseCoordinate('', -90, 90)).toBeNull();
    expect(parseCoordinate('north', -90, 90)).toBeNull();
  });

  it('name the preset of a position and the offline state', () => {
    expect(presetOf(null)).toBeNull();
    expect(presetOf({ ...base, geolocation: { latitude: 52.52, longitude: 13.405, accuracy: 50 } })).toBe(
      'berlin',
    );
    expect(presetOf({ ...base, geolocation: { latitude: 1, longitude: 2, accuracy: 50 } })).toBeNull();
    expect(isOffline(base)).toBe(false);
    expect(
      isOffline({ ...base, network: { offline: true, latencyMs: 0, downloadKbps: 0, uploadKbps: 0 } }),
    ).toBe(true);
  });
});
