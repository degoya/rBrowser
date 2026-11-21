// The portal's button in the toolbar (RB-090-62): its three states, the name of each, the rows of
// its native menu while the portal runs and what each row runs - the portal's existing commands.
import { describe, expect, it } from 'vitest';
import type { PortalDevice, PortalState } from '../ipc/protocol';
import {
  chipText,
  portalChoice,
  portalLabel,
  portalLook,
  portalRows,
  PORTAL_DEVICES_COMMAND,
} from './portalButton';

/** `t` as the tests see it: the key and its arguments. */
const t = (key: string, args: Record<string, unknown> = {}): string =>
  Object.keys(args).length === 0 ? key : `${key}${JSON.stringify(args)}`;

function device(id: number, name: string, ip = `192.168.1.${id}`): PortalDevice {
  return {
    id,
    name,
    ip,
    pairedAtMs: 0,
    lastSeenMs: 0,
    contacted: true,
    connected: true,
    replayed: 0,
    failed: 0,
    skipped: 0,
  };
}

function running(devices: PortalDevice[] = []): PortalState {
  return { enabled: true, running: true, address: 'http://192.168.1.10:8890', devices };
}

describe("the portal's button", () => {
  it('is off without a state, while the portal may not run and while it does not listen', () => {
    expect(portalLook(null)).toBe('off');
    expect(portalLook({ enabled: false, running: false, devices: [] })).toBe('off');
    expect(portalLook({ enabled: true, running: false, devices: [] })).toBe('off');
  });

  it('is on without a device, and on with the paired devices', () => {
    expect(portalLook(running())).toBe('idle');
    expect(portalLook(running([device(1, 'Pixel')]))).toBe('paired');
  });

  it('names each state, the number of devices included', () => {
    expect(portalLabel(null, t)).toBe('shell.toolbar.portal.off');
    expect(portalLabel(running(), t)).toBe('shell.toolbar.portal.idle');
    expect(portalLabel(running([device(1, 'Pixel')]), t)).toBe('shell.toolbar.portal.paired{"count":1}');
    expect(portalLabel(running([device(1, 'Pixel'), device(2, 'iPad')]), t)).toBe(
      'shell.toolbar.portal.paired{"count":2}',
    );
  });

  it('puts the number of devices on its chip, at most two characters', () => {
    expect(chipText(1)).toBe('1');
    expect(chipText(9)).toBe('9');
    expect(chipText(10)).toBe('9+');
  });
});

describe("the portal button's menu", () => {
  it('offers nothing while the portal is off: the click starts it', () => {
    expect(portalRows(null, t)).toEqual([]);
    expect(portalRows({ enabled: true, running: false, devices: [] }, t)).toEqual([]);
  });

  it('shows the QR code, the devices, one device to disconnect and the stop', () => {
    const rows = portalRows(running([device(3, 'Pixel 8'), device(4, '', '192.168.1.40')]), t);
    expect(rows).toEqual([
      { kind: 'item', id: 'show_qr', label: 'shell.toolbar.portal.menu.show_qr' },
      { kind: 'item', id: 'devices', label: 'shell.toolbar.portal.menu.devices' },
      { kind: 'separator' },
      { kind: 'heading', label: 'shell.toolbar.portal.menu.disconnect' },
      {
        kind: 'item',
        id: 'device:3',
        label: 'shell.toolbar.portal.menu.device{"name":"Pixel 8","ip":"192.168.1.3"}',
      },
      {
        kind: 'item',
        id: 'device:4',
        label: 'shell.toolbar.portal.menu.device{"name":"portal.panel.unknown_device","ip":"192.168.1.40"}',
      },
      { kind: 'item', id: 'device:all', label: 'shell.toolbar.portal.menu.disconnect_all' },
      { kind: 'separator' },
      { kind: 'item', id: 'stop', label: 'portal.panel.stop' },
    ]);
  });

  it('offers no "all devices" for one device, and says so when none is paired', () => {
    const one = portalRows(running([device(3, 'Pixel 8')]), t);
    expect(one.filter((row) => row.kind === 'item').map((row) => (row.kind === 'item' ? row.id : ''))).toEqual([
      'show_qr',
      'devices',
      'device:3',
      'stop',
    ]);
    const none = portalRows(running(), t);
    expect(none).toContainEqual({
      kind: 'item',
      id: 'device:none',
      label: 'portal.panel.no_devices',
      disabled: true,
    });
  });

  it('runs the portal commands for each row', () => {
    expect(portalChoice('show_qr')).toEqual({ type: 'runCommand', id: 'portal.show_qr' });
    expect(portalChoice('devices')).toEqual({ type: 'runCommand', id: PORTAL_DEVICES_COMMAND });
    expect(PORTAL_DEVICES_COMMAND).toBe('portal.devices');
    expect(portalChoice('device:7')).toEqual({ type: 'runCommand', id: 'portal.disconnect', args: { device: 7 } });
    expect(portalChoice('device:all')).toEqual({ type: 'runCommand', id: 'portal.disconnect' });
    expect(portalChoice('stop')).toEqual({ type: 'runCommand', id: 'portal.toggle', args: { enabled: false } });
  });

  it('runs nothing for a row it does not know', () => {
    for (const id of ['device:none', 'device:', 'device:x', 'device:-1', 'device:1.5', 'other', '']) {
      expect(portalChoice(id), id).toBeNull();
    }
  });

  it('starts or ends the presentation over the internet where a person allowed it (RB-096-05)', () => {
    const allowed: PortalState = {
      ...running(),
      presentation: { enabled: true, consented: false, phase: 'off', viewers: 0, maxViewers: 3 },
    };
    const ids = (rows: ReturnType<typeof portalRows>) =>
      rows.filter((row) => row.kind === 'item').map((row) => (row.kind === 'item' ? row.id : ''));
    expect(ids(portalRows(allowed, t))).toContain('present');
    const live: PortalState = { ...allowed, presentation: { ...allowed.presentation!, phase: 'live' } };
    expect(ids(portalRows(live, t))).toContain('present_stop');
    const off: PortalState = { ...allowed, presentation: { ...allowed.presentation!, enabled: false } };
    expect(ids(portalRows(off, t))).not.toContain('present');
    expect(portalChoice('present')).toEqual({ type: 'runCommand', id: 'portal.present.start', args: {} });
    expect(portalChoice('present_stop')).toEqual({ type: 'runCommand', id: 'portal.present.stop' });
  });
});
