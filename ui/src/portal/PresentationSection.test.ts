import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createShellI18n } from '../i18n';
import type { PortalPresentation, ShellToCore } from '../ipc/protocol';
import { settle } from '../settings/test/mountPage';
import PresentationSection from './PresentationSection.vue';

const LINK = 'https://quick-brown-fox.trycloudflare.com/__rbrowser/join#t=00ff';

const LIVE: PortalPresentation = {
  enabled: true,
  consented: true,
  phase: 'live',
  address: 'https://quick-brown-fox.trycloudflare.com',
  viewers: 2,
  maxViewers: 3,
  link: { url: LINK, size: 2, rows: ['10', '01'], expiresAtMs: Date.now() + 600_000 },
};

async function mountSection(presentation: PortalPresentation) {
  const clipboard = { writeText: vi.fn(() => Promise.resolve()) };
  const wrapper = mount(PresentationSection, {
    props: { presentation, nowMs: Date.now(), clipboard },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return {
    wrapper,
    clipboard,
    sent: () => (wrapper.emitted('run') ?? []).map(([message]) => message as ShellToCore),
  };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe('the presentation over the internet in the Portal panel (RB-096-05)', () => {
  it('says it is off while portal.presentation.enabled is off', async () => {
    const { wrapper } = await mountSection({ ...LIVE, enabled: false, phase: 'off', link: undefined });
    expect(wrapper.find('[data-testid="presentation-disabled"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="presentation-start"]').exists()).toBe(false);
  });

  // RB-096-09 (owner): the hint opens Settings at the key instead of naming the way there.
  it('opens Settings at portal.presentation.enabled from the off hint', async () => {
    const { wrapper, sent } = await mountSection({ ...LIVE, enabled: false, phase: 'off', link: undefined });
    const button = wrapper.get('[data-testid="presentation-open-setting"]');
    expect(button.text()).toBe('Open the setting');
    await button.trigger('click');
    expect(sent()).toEqual([{ type: 'openSettings', key: 'portal.presentation.enabled' }]);
  });

  it('asks for the consent first, naming Cloudflare, the terms and the download', async () => {
    const { wrapper, sent } = await mountSection({
      enabled: true,
      consented: false,
      phase: 'off',
      viewers: 0,
      maxViewers: 3,
      download: { version: '2026.11.1', sizeMb: '55.5', license: 'Apache-2.0', from: 'github.com' },
    });
    const consent = wrapper.get('[data-testid="presentation-consent"]');
    expect(consent.text()).toContain('Cloudflare');
    expect(wrapper.get('[data-testid="presentation-terms"]').text()).toContain('demonstrations');
    expect(wrapper.get('[data-testid="presentation-download"]').text()).toContain('55.5');
    await wrapper.get('[data-testid="presentation-start"]').trigger('click');
    expect(sent()).toContainEqual({
      type: 'runCommand',
      id: 'portal.present.start',
      args: { consent: true, download: true },
    });
  });

  it('starts without asking again once agreed', async () => {
    const { wrapper, sent } = await mountSection({
      enabled: true,
      consented: true,
      phase: 'off',
      viewers: 0,
      maxViewers: 3,
    });
    expect(wrapper.find('[data-testid="presentation-consent"]').exists()).toBe(false);
    await wrapper.get('[data-testid="presentation-start"]').trigger('click');
    expect(sent()).toContainEqual({ type: 'runCommand', id: 'portal.present.start', args: {} });
  });

  it('shows the live link with its QR code, copies it and ends the presentation', async () => {
    const { wrapper, sent, clipboard } = await mountSection(LIVE);
    expect(wrapper.get('[data-testid="presentation-status"]').text()).toContain('2');
    expect(wrapper.get('[data-testid="presentation-qr"] path').attributes('d')).toBe('M0 0h1v1h-1zM1 1h1v1h-1z');
    await wrapper.get('[data-testid="presentation-copy"]').trigger('click');
    await settle();
    expect(clipboard.writeText).toHaveBeenCalledWith(LINK);
    await wrapper.get('[data-testid="presentation-new-link"]').trigger('click');
    await wrapper.get('[data-testid="presentation-stop"]').trigger('click');
    expect(sent()).toContainEqual({ type: 'runCommand', id: 'portal.present.new_link' });
    expect(sent()).toContainEqual({ type: 'runCommand', id: 'portal.present.stop' });
  });

  it('offers a new link once the open one is spent', async () => {
    const { wrapper } = await mountSection({ ...LIVE, link: undefined });
    expect(wrapper.find('[data-testid="presentation-qr"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="presentation-no-link"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="presentation-new-link"]').exists()).toBe(true);
  });

  it('can end a presentation whose tunnel still opens', async () => {
    const { wrapper } = await mountSection({ ...LIVE, phase: 'starting', link: undefined, address: undefined });
    expect(wrapper.find('[data-testid="presentation-starting"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="presentation-stop"]').exists()).toBe(true);
  });
});
