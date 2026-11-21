// The check progress as the owner's grid shows it (RB-080-98): eleven plugins over nine views
// in one row per plugin, the running ones on top, only the views worth naming in the row, the
// rest on demand; a view of another engine skipped with its sentence; after the run one line.
import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { createShellI18n } from '../i18n';
import { settle } from '../test/mountShell';
import FindingsProgress from './FindingsProgress.vue';
import { nestMessages } from './findings';
import { GRID_PLUGINS, GRID_VIEWS, gridProgress } from './testing';
import type { ChecksProgress } from './types';

function mountProgress(progress: ChecksProgress = gridProgress(), texts: Record<string, string> = {}) {
  const i18n = createShellI18n();
  i18n.global.mergeLocaleMessage('en', nestMessages(texts));
  return mount(FindingsProgress, {
    props: { progress, elapsedMs: progress.elapsedMs },
    attachTo: document.body,
    global: { plugins: [ui, i18n] },
  });
}

function rowOf(wrapper: ReturnType<typeof mountProgress>, plugin: string) {
  return wrapper.get(`[data-testid="progress-${plugin}"]`);
}

describe('FindingsProgress over eleven plugins and nine views', () => {
  it('draws one row per plugin, the running ones on top, then the waiting ones', async () => {
    const wrapper = mountProgress();
    await settle();
    const plugins = wrapper.findAll('[data-plugin]').map((row) => row.attributes('data-plugin'));
    expect(plugins).toHaveLength(GRID_PLUGINS.length);
    expect(plugins.slice(0, 3)).toEqual(['error-report', 'fonts', 'lighthouse']);
    expect(plugins.slice(3, 8)).toEqual(['outline', 'pwa-check', 'seo', 'visual-regression', 'links']);
    expect(wrapper.get('[data-testid="progress"]').text()).toContain('47 of 99 checks');
    wrapper.unmount();
  });

  it('sums a plugin up in its row and names only the views worth naming', async () => {
    const wrapper = mountProgress();
    await settle();
    const lighthouse = rowOf(wrapper, 'lighthouse').text();
    expect(lighthouse).toContain('Running');
    expect(lighthouse).toContain('4/9 views');
    expect(lighthouse).toContain('66 findings');
    expect(lighthouse).toContain('Running: laptop, w1600');
    expect(lighthouse).toContain('Skipped: mobile');
    expect(lighthouse).not.toContain('w1920');
    const console = rowOf(wrapper, 'console');
    expect(console.text()).toContain('Done');
    expect(console.text()).toContain('9/9 views');
    expect(console.text()).toContain('Nothing found');
    expect(console.findAll('[data-testid="attention"]')).toHaveLength(0);
    expect(rowOf(wrapper, 'a11y-axe').text()).toContain('Failed: mobile');
    expect(rowOf(wrapper, 'outline').text()).toContain('Waiting');
    // No row lists its nine views: the quiet ones wait behind the row.
    const text = wrapper.get('[data-testid="progress"]').text();
    for (const breakpoint of ['md', 'lg', 'xl', 'w1920', 'w2000', 'desktop']) {
      expect(text).not.toMatch(new RegExp(`\\b${breakpoint}\\b`));
    }
    expect(wrapper.find('[data-testid="views-console"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('opens a plugin to every view, a skipped one with its sentence and no raw placeholder', async () => {
    const wrapper = mountProgress();
    await settle();
    // The row is the accordion's trigger: a button with the chevron.
    const trigger = rowOf(wrapper, 'coverage').element.closest('button');
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');
    trigger?.click();
    await settle();
    const views = wrapper.get('[data-testid="views-coverage"]');
    expect(views.findAll('li')).toHaveLength(GRID_VIEWS.length);
    const mobile = views.get('[data-view="mobile"]');
    expect(mobile.text()).toContain('Skipped');
    expect(mobile.text()).toContain('This check needs a Chromium view; this view is WebKit.');
    expect(mobile.classes().join(' ')).not.toContain('text-error');
    expect(views.get('[data-view="md"]').text()).toContain('17 findings');
    expect(wrapper.text()).not.toContain('{');
    wrapper.unmount();
  });

  it('counts the views when more than three share a state', async () => {
    const progress = gridProgress();
    progress.jobs = progress.jobs.map((job) =>
      job.plugin === 'outline' ? { ...job, state: 'skipped', reason: 'checks.skip.cancelled' } : job,
    );
    const wrapper = mountProgress(progress);
    await settle();
    expect(rowOf(wrapper, 'outline').text()).toContain('Skipped: 9 views');
    wrapper.unmount();
  });

  it('cancels', async () => {
    const wrapper = mountProgress();
    await settle();
    await wrapper.get('[data-testid="cancel"]').trigger('click');
    expect(wrapper.emitted('cancel')).toHaveLength(1);
    wrapper.unmount();
  });

  it('collapses to one summary line after the run, the rows on demand', async () => {
    const wrapper = mountProgress(gridProgress({ finished: true }));
    await settle();
    expect(wrapper.find('[data-testid="progress"]').exists()).toBe(false);
    const summary = wrapper.get('[data-testid="progress-summary"]');
    expect(summary.text()).toContain('99 checks in 35 s');
    expect(summary.text()).toContain('1 failed');
    expect(summary.text()).toContain('3 skipped');
    expect(wrapper.find('[data-plugin]').exists()).toBe(false);
    const details = wrapper.get('[data-testid="progress-details"]');
    expect(details.attributes('aria-expanded')).toBe('false');
    await details.trigger('click');
    await settle();
    expect(details.attributes('aria-expanded')).toBe('true');
    expect(wrapper.findAll('[data-plugin]')).toHaveLength(GRID_PLUGINS.length);
    wrapper.unmount();
  });

  it('announces the count and the end of the run, never the ticking time or the Cancel button (RB-090-22)', async () => {
    const progress = gridProgress();
    const wrapper = mountProgress(progress);
    await settle();
    const live = (): string[] =>
      wrapper.findAll('[role="status"], [role="alert"], [aria-live]').map((region) => region.text());
    expect(live()).toEqual(['47 of 99 checks']);
    expect(wrapper.find('[aria-live] button, [role="status"] button').exists()).toBe(false);
    await wrapper.setProps({ elapsedMs: progress.elapsedMs + 3000 });
    await settle();
    expect(live()).toEqual(['47 of 99 checks']);
    const status = wrapper.get('[data-testid="progress-status"]').element;
    await wrapper.setProps({ progress: gridProgress({ finished: true }), elapsedMs: progress.elapsedMs });
    await settle();
    expect(live()).toEqual(['99 checks in 35 s']);
    // The same region, so the change is announced, not a new region with its text.
    expect(wrapper.get('[data-testid="progress-status"]').element).toBe(status);
    wrapper.unmount();
  });

  it('says a cancelled run was cancelled, not done', async () => {
    const progress = gridProgress({ finished: true, cancelled: true });
    const wrapper = mountProgress(progress);
    await settle();
    const summary = wrapper.get('[data-testid="progress-summary"]');
    expect(summary.text()).toContain('Cancelled: 47 of 99 checks in 35 s');
    expect(summary.find('.text-success').exists()).toBe(false);
    wrapper.unmount();
  });

  it('names the plugin itself in a reason, never what the plugin put there, and no raw engine text', async () => {
    const progress = gridProgress();
    progress.jobs = progress.jobs.map((job) =>
      job.plugin === 'a11y-axe' && job.breakpoint === 'mobile'
        ? { ...job, reason: 'plugins.error.crashed', reasonParams: { id: 'someone-else', reason: 'TypeError: x at page.js' } }
        : job,
    );
    const wrapper = mountProgress(progress);
    await settle();
    rowOf(wrapper, 'a11y-axe').element.closest('button')?.click();
    await settle();
    const mobile = wrapper.get('[data-testid="views-a11y-axe"] [data-view="mobile"]').text();
    expect(mobile).toContain('The plugin a11y-axe crashed');
    expect(mobile).not.toContain('someone-else');
    expect(mobile).not.toContain('TypeError');
    wrapper.unmount();
  });

  it("gives a plugin's own failure its reason", async () => {
    const progress = gridProgress();
    progress.jobs = progress.jobs.map((job) =>
      job.plugin === 'a11y-axe' && job.breakpoint === 'mobile'
        ? { ...job, state: 'failed', reason: 'plugin.a11y-axe.errors.no_page', reasonParams: { reason: 'blank page' } }
        : job,
    );
    const wrapper = mountProgress(progress, { 'plugin.a11y-axe.errors.no_page': 'No page to check: {reason}' });
    await settle();
    rowOf(wrapper, 'a11y-axe').element.closest('button')?.click();
    await settle();
    const mobile = wrapper.get('[data-testid="views-a11y-axe"] [data-view="mobile"]').text();
    expect(mobile).toContain('No page to check: blank page');
    wrapper.unmount();
  });
});
