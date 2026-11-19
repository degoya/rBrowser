import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import type { ShellToCore } from '../ipc/protocol';
import { settle } from '../settings/test/mountPage';
import SetupWizard from './SetupWizard.vue';
import { fakeCore, SAMPLE_PLUGINS } from './testing';

function core(enabled = true, choicePending = true) {
  return fakeCore((message: ShellToCore) => {
    if (message.type === 'setup' && message.request.op === 'info') {
      return { type: 'setup', answer: { op: 'info', enabled } };
    }
    if (message.type === 'pluginPanel') {
      return { type: 'pluginPanel', message: { value: { offline: false, choicePending, plugins: SAMPLE_PLUGINS } } };
    }
    if (message.type === 'getDevices') {
      return { type: 'devices', items: [] };
    }
    return undefined;
  });
}

async function mountWizard(fake = core(), routeKey: string | null = null) {
  const wrapper = mount(SetupWizard, {
    props: { client: fake.client, routeKey },
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()] },
  });
  await settle();
  return wrapper;
}

const pluginOps = (sent: ShellToCore[]) =>
  sent.flatMap((message) => (message.type === 'pluginPanel' ? [(message.request as { op: string }).op] : []));

const closes = (sent: ShellToCore[]) =>
  sent.filter((message) => message.type === 'setup' && message.request.op === 'close').length;

afterEach(() => {
  document.body.replaceChildren();
  window.location.hash = '';
});

describe('the setup wizard', () => {
  it('starts with the welcome, and "Just start" closes it without any change', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    expect(wrapper.find('[data-testid="setup-welcome"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-just-start"]').trigger('click');
    await settle();
    expect(closes(fake.sent)).toBe(1);
    // Pending was not known yet: asked, then the recommended set applied.
    expect(pluginOps(fake.sent)).toEqual(['available', 'applyRecommended']);
    expect(fake.sent.some((message) => message.type === 'runCommand' || message.type === 'setSetting')).toBe(false);
    wrapper.unmount();
  });

  it('walks every step with Next, each skippable, and ends with the summary', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setup-plugins"]').exists()).toBe(true);
    for (const next of ['setup-project', 'setup-basics', 'setup-done']) {
      await wrapper.get('[data-testid="setup-next"]').trigger('click');
      await settle();
      expect(wrapper.find(`[data-testid="${next}"]`).exists()).toBe(true);
    }
    expect(wrapper.get('[data-testid="setup-summary"]').text()).toContain('Nothing was changed');
    // Nothing was installed or written on the way; passing the plugin step leaves the
    // recommended set to the core, once.
    expect(pluginOps(fake.sent)).toEqual(['available', 'applyRecommended']);
    expect(fake.sent.some((message) => message.type === 'setup' && message.request.op === 'write')).toBe(false);
    await wrapper.get('[data-testid="setup-finish"]').trigger('click');
    await settle();
    expect(closes(fake.sent)).toBe(1);
    expect(pluginOps(fake.sent).filter((op) => op === 'applyRecommended')).toHaveLength(1);
    wrapper.unmount();
  });

  it('scrolls itself, with Back and Next kept in sight', async () => {
    // The pages window's body never scrolls (assets/main.css); a long plugin list must not hide
    // the footer (owner, 2026-11-18).
    const wrapper = await mountWizard();
    expect(wrapper.get('[data-testid="setup-scroll"]').classes()).toEqual(expect.arrayContaining(['h-screen', 'overflow-y-auto']));
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    expect(wrapper.get('footer').classes()).toContain('sticky');
  });

  it('offers "Skip setup" on every page', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    await settle();
    expect(wrapper.find('[data-testid="setup-skip"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-skip"]').trigger('click');
    await settle();
    expect(closes(fake.sent)).toBe(1);
    // Skipped before the plugins were answered: the core installs its recommended set.
    expect(pluginOps(fake.sent)).toContain('applyRecommended');
    wrapper.unmount();
  });

  it('opens at the project step from the hint, and leaves the plugins as they are', async () => {
    const fake = core();
    const wrapper = await mountWizard(fake, 'project');
    expect(wrapper.find('[data-testid="setup-project"]').exists()).toBe(true);
    await wrapper.get('[data-testid="setup-skip"]').trigger('click');
    await settle();
    expect(pluginOps(fake.sent)).toEqual([]);
    wrapper.unmount();
  });

  it('says it is switched off and offers only Close', async () => {
    const wrapper = await mountWizard(core(false));
    expect(wrapper.find('[data-testid="setup-off"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="setup-stepper"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="setup-close"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('opens a place in the settings from the last step', async () => {
    const wrapper = await mountWizard(core());
    await wrapper.get('[data-testid="setup-start"]').trigger('click');
    for (let i = 0; i < 3; i += 1) {
      await settle();
      await wrapper.get('[data-testid="setup-next"]').trigger('click');
    }
    await settle();
    await wrapper.get('[data-testid="setup-where-ui.firstRunWizard"]').trigger('click');
    expect(window.location.hash).toBe('#/settings/ui.firstRunWizard');
    wrapper.unmount();
  });

  it('leaves a choice made before as it is when skipped', async () => {
    const fake = core(true, false);
    const wrapper = await mountWizard(fake);
    await wrapper.get('[data-testid="setup-just-start"]').trigger('click');
    await settle();
    expect(pluginOps(fake.sent)).toEqual(['available']);
    expect(closes(fake.sent)).toBe(1);
    wrapper.unmount();
  });
});
