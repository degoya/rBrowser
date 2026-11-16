import { afterEach, describe, expect, it } from 'vitest';
import ui from '@nuxt/ui/vue-plugin';
import { mount, type VueWrapper } from '@vue/test-utils';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import { fakeTrustCore, ROOT, type FakePlugin } from './testCore';
import { createTrust, TRUST_KEY } from './trust';
import TrustBadge from './TrustBadge.vue';
import TrustPage from './TrustPage.vue';

let current: VueWrapper | null = null;

function mountWith(
  component: typeof TrustPage | typeof TrustBadge,
  plugin?: FakePlugin,
) {
  const core = fakeTrustCore(plugin);
  const trust = createTrust(core.client);
  const wrapper = mount(component, {
    attachTo: document.body,
    global: { plugins: [ui, createShellI18n()], provide: { [TRUST_KEY]: trust } },
  });
  current = wrapper;
  return { wrapper, core, trust };
}

afterEach(() => {
  current?.unmount();
  current = null;
});

describe('the trust dialog', () => {
  it("lists a project plugin's capabilities under its package", async () => {
    const { wrapper } = mountWith(TrustPage, {
      path: '.rbrowser/plugins/seo.rbplugin',
      capabilities: ['http', 'views'],
    });
    await settle();
    const listed = wrapper.findAll('[data-testid="trust-capabilities"] li');
    expect(listed.map((item) => item.text())).toEqual([
      'Fetch web addresses (rate-limited)',
      'Read the views and their sizes',
    ]);
  });

  it("shows a project plugin's grants in detail and highlights the risky ones", async () => {
    const { wrapper } = mountWith(TrustPage, {
      path: '.rbrowser/plugins/audit.rbplugin',
      capabilities: ['cdp', 'evaluate', 'http', 'project-read', 'sidecar'],
      grants: {
        evaluate: true,
        cdpDomains: ['CSS', 'Network'],
        projectGlobs: ['src/**/*.css'],
        sidecarMethods: ['lighthouse.run'],
        httpRatePerSecond: 4,
        risky: ['evaluate', 'cdp:Network'],
      },
    });
    await settle();
    const grants = wrapper.get('[data-testid="trust-grants"]');
    const text = grants.text();
    expect(text).toContain('full control of the page');
    expect(text).toContain('src/**/*.css');
    expect(text).toContain('lighthouse.run');
    expect(text).toContain('at most 4 per second');
    const risky = grants.findAll('[data-risky="true"]').map((item) => item.text());
    expect(risky).toHaveLength(2);
    expect(risky[0]).toContain('full control of the page');
    expect(risky[1]).toBe('Network');
    const css = grants.findAll('code').find((item) => item.text() === 'CSS');
    expect(css?.attributes('data-risky')).toBeUndefined();
    // Told without its colour and without a hover: a word and the reason beside each (RB-090-22).
    const notes = grants.findAll('[data-testid="trust-risky-note"]').map((note) => note.text());
    expect(notes).toEqual(['Risky', 'Risky: Controls the page or reads its traffic']);
    expect(grants.find('[title]').exists()).toBe(false);
  });

  it('highlights the network-rules and console-logs capabilities (review L3 of Charge 7)', async () => {
    const { wrapper } = mountWith(TrustPage, {
      path: '.rbrowser/plugins/mocks.rbplugin',
      capabilities: ['console-logs', 'network-rules', 'views'],
      grants: {
        evaluate: false,
        cdpDomains: [],
        projectGlobs: [],
        sidecarMethods: [],
        risky: ['network-rules', 'console-logs'],
      },
    });
    await settle();
    const listed = wrapper.findAll('[data-testid="trust-capabilities"] li');
    const risky = listed.filter((item) => item.attributes('data-risky') === 'true');
    expect(risky.map((item) => item.get('[data-testid="trust-capability"]').text())).toEqual([
      "Read the views' console messages, errors and network requests",
      'Supply network rules (mocks, blocking)',
    ]);
    expect(listed[2]?.attributes('data-risky')).toBeUndefined();
    // The reason is read, not hovered for (RB-090-22).
    for (const item of risky) {
      expect(item.get('[data-testid="trust-risky-note"]').text()).toBe(
        'Risky: Sees or changes every request and console message of the views',
      );
      expect(item.attributes('title')).toBeUndefined();
    }
    expect(listed[2]?.find('[data-testid="trust-risky-note"]').exists()).toBe(false);
  });

  it('highlights project-write and shows the files it may change (RB-110-13)', async () => {
    const { wrapper } = mountWith(TrustPage, {
      path: '.rbrowser/plugins/notes.rbplugin',
      capabilities: ['project-write'],
      grants: {
        evaluate: false,
        cdpDomains: [],
        projectGlobs: [],
        writeGlobs: ['**/*.todo'],
        sidecarMethods: [],
        risky: ['project-write'],
      },
    });
    await settle();
    const capability = wrapper.get('[data-testid="trust-capabilities"] li');
    expect(capability.attributes('data-risky')).toBe('true');
    expect(capability.get('[data-testid="trust-risky-note"]').text()).toBe(
      'Risky: Writes into your project files, not only into its own output folder',
    );
    const grants = wrapper.get('[data-testid="trust-grants"]');
    const write = grants.get('[data-risky="true"]');
    expect(write.text()).toContain('Changes project files:');
    expect(write.text()).toContain('**/*.todo');
  });

  it('highlights the sidecar capability and its methods (security review RB-090-06)', async () => {
    const { wrapper } = mountWith(TrustPage, {
      path: '.rbrowser/plugins/audit.rbplugin',
      capabilities: ['sidecar', 'views'],
      grants: {
        evaluate: false,
        cdpDomains: [],
        projectGlobs: [],
        sidecarMethods: ['lighthouse.run'],
        risky: ['sidecar'],
      },
    });
    await settle();
    const listed = wrapper.findAll('[data-testid="trust-capabilities"] li');
    const risky = listed.filter((item) => item.attributes('data-risky') === 'true');
    expect(risky).toHaveLength(1);
    expect(risky[0]?.text()).toContain('Use the Node sidecar (Lighthouse, Playwright)');
    // The reason names the limit, wherever the item shows it.
    expect(risky[0]?.html()).toContain('sites the views show');
    const methods = wrapper
      .get('[data-testid="trust-grants"]')
      .findAll('li')
      .find((item) => item.text().includes('lighthouse.run'));
    expect(methods?.attributes('data-risky')).toBe('true');
  });

  it('shows no grant details for a package without any', async () => {
    const { wrapper } = mountWith(TrustPage, {
      path: '.rbrowser/plugins/seo.rbplugin',
      capabilities: ['views'],
    });
    await settle();
    expect(wrapper.find('[data-testid="trust-grants"]').exists()).toBe(false);
  });

  it('lists the blocked parts and trusts only on its button', async () => {
    const { wrapper, core } = mountWith(TrustPage);
    await settle();
    expect(wrapper.get('[data-testid="trust-root"]').text()).toBe(ROOT);
    expect(wrapper.get('[data-testid="trust-status"]').text()).toContain('Restricted');
    const items = wrapper.findAll('[data-testid="trust-blocked"] li');
    expect(items.map((item) => item.text())).toEqual([
      'Flow.rbrowser/flows/login.ts',
      'Host resolver rules/network/hostResolverRules',
    ]);
    expect(wrapper.text()).toContain('starts again');
    expect(core.sent.some((m) => m.type === 'trustDecision')).toBe(false);

    await wrapper.get('[data-testid="trust-grant"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'trustDecision', promptId: 7, decision: 'trust' });
    expect(wrapper.get('[data-testid="trust-status"]').text()).toContain('Trusted');
    expect(wrapper.find('[data-testid="trust-grant"]').exists()).toBe(false);
    expect(wrapper.findAll('[data-testid="trust-projects"] li')).toHaveLength(1);
  });

  it('stays restricted on the second button', async () => {
    const { wrapper, core } = mountWith(TrustPage);
    await settle();
    await wrapper.get('[data-testid="trust-restricted"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({
      type: 'trustDecision',
      promptId: 7,
      decision: 'restricted',
    });
    expect(wrapper.get('[data-testid="trust-status"]').text()).toContain('Restricted');
  });

  it('revokes a trusted project', async () => {
    const { wrapper, core } = mountWith(TrustPage);
    await settle();
    await wrapper.get('[data-testid="trust-grant"]').trigger('click');
    await settle();
    const revoke = wrapper.get('[data-testid="trust-revoke"]');
    expect(revoke.attributes('aria-label')).toBe(`Revoke the trust of ${ROOT}`);
    await revoke.trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'revokeTrust', root: ROOT });
    expect(wrapper.text()).toContain('No project is trusted.');
  });
});

describe('the trust badge', () => {
  it('shows the blocked count and opens the dialog', async () => {
    const { wrapper, core, trust } = mountWith(TrustBadge);
    expect(wrapper.find('[data-testid="trust-badge"]').exists()).toBe(false);
    trust.start();
    core.prompt();
    await settle();
    const badge = wrapper.get('[data-testid="trust-badge"]');
    expect(badge.text()).toBe('Restricted: 2 blocked');
    await badge.trigger('click');
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'trust.show', args: {} });
  });
});
