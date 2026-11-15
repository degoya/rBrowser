import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { IpcClient } from '../ipc/client';
import type { CoreToShell, PortalState, ShellToCore } from '../ipc/protocol';
import type { Transport } from '../ipc/transport';
import { settle } from '../settings/test/mountPage';
import PortalPage from './PortalPage.vue';

const RUNNING: PortalState = {
  enabled: true,
  running: true,
  address: 'http://192.168.1.20:4000',
  devices: [
    {
      id: 7,
      name: 'iPhone',
      ip: '192.168.1.31',
      pairedAtMs: 0,
      lastSeenMs: 0,
      contacted: true,
      connected: true,
      replayed: 1,
      failed: 0,
      skipped: 2,
    },
  ],
  pairing: {
    url: 'http://192.168.1.20:4000/__rbrowser/pair?t=00',
    size: 2,
    rows: ['10', '01'],
    expiresAtMs: Date.now() + 300_000,
  },
};

function fakeCore(state: () => PortalState) {
  const sent: ShellToCore[] = [];
  const listeners: Array<(message: CoreToShell) => void> = [];
  const transport: Transport = {
    send: (message) => {
      sent.push(message);
      return Promise.resolve(
        message.type === 'getPortal' ? { type: 'portal', ...state() } : { type: 'ack' },
      );
    },
    subscribe: (listener) => {
      listeners.push(listener);
      return () => undefined;
    },
  };
  return {
    client: new IpcClient(transport),
    sent,
    emit: (message: CoreToShell) => listeners.forEach((listener) => listener(message)),
  };
}

async function mountPortal(state: () => PortalState) {
  const core = fakeCore(state);
  const wrapper = mount(PortalPage, {
    props: { client: core.client },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return { wrapper, core };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe('the Portal panel', () => {
  it('says the portal is off while portal.enabled is off', async () => {
    const { wrapper } = await mountPortal(() => ({ enabled: false, running: false, devices: [] }));
    expect(wrapper.find('[data-testid="portal-disabled"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="portal-toggle"]').exists()).toBe(false);
  });

  it('draws the QR code from its modules and lists the devices', async () => {
    const { wrapper } = await mountPortal(() => RUNNING);
    const path = wrapper.find('[data-testid="portal-qr"] path');
    expect(path.attributes('d')).toBe('M0 0h1v1h-1zM1 1h1v1h-1z');
    expect(wrapper.find('[data-testid="portal-qr"]').html()).not.toContain('pair?t=');
    expect(wrapper.find('[data-testid="portal-device-7"]').text()).toContain('192.168.1.31');
  });

  it('names a device that never came back with its cookie as pairing not completed', async () => {
    const device = { ...RUNNING.devices[0]!, id: 8, contacted: false, connected: false };
    const { wrapper } = await mountPortal(() => ({ ...RUNNING, devices: [device] }));
    expect(wrapper.find('[data-testid="portal-device-8"]').text()).toContain('pairing not completed');
  });

  it('disconnects a device and asks again', async () => {
    const { wrapper, core } = await mountPortal(() => RUNNING);
    await wrapper.find('[data-testid="portal-disconnect-7"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({
      type: 'runCommand',
      id: 'portal.disconnect',
      args: { device: 7 },
    });
    expect(core.sent.filter((message) => message.type === 'getPortal').length).toBeGreaterThan(1);
  });

  it('asks again on portalChanged and hides a spent code', async () => {
    let state: PortalState = RUNNING;
    const { wrapper, core } = await mountPortal(() => state);
    state = { ...RUNNING, pairing: undefined };
    core.emit({ type: 'portalChanged' });
    await settle();
    expect(wrapper.find('[data-testid="portal-qr"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="portal-no-code"]').exists()).toBe(true);
  });

  it('stops a running portal', async () => {
    const { wrapper, core } = await mountPortal(() => RUNNING);
    await wrapper.find('[data-testid="portal-toggle"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({
      type: 'runCommand',
      id: 'portal.toggle',
      args: { enabled: false },
    });
  });
});
