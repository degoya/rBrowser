// RB-080-73: "Save diagnostics..." in Settings > Advanced > Diagnostics and on About.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import DiagnosticsSave from './DiagnosticsSave.vue';
import { createFakeCore } from './test/fakeCore';
import { mountPage, settle } from './test/mountPage';

let current: VueWrapper | null = null;

afterEach(() => {
  current?.unmount();
  current = null;
});

async function open() {
  const core = createFakeCore();
  const mounted = await mountPage(DiagnosticsSave, {}, core);
  current = mounted.wrapper;
  return mounted;
}

describe('save diagnostics', () => {
  it('says what the ZIP holds and that nothing is sent', async () => {
    const { wrapper } = await open();
    expect(wrapper.get('[data-testid="diagnostics-save"]').text()).toBe('Save diagnostics…');
    expect(wrapper.text()).toContain('Nothing is sent anywhere');
  });

  it('runs diagnostics.create, waits for the dialog and shows the path with Show in folder', async () => {
    const { wrapper, core } = await open();
    await wrapper.get('[data-testid="diagnostics-save"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'runCommand', id: 'diagnostics.create' });
    expect(wrapper.get('[data-testid="diagnostics-save"]').attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="diagnostics-status"]').text()).toBe('Choose where to save the ZIP…');

    core.emit({ type: 'diagnosticsSaved', path: 'C:\\Users\\me\\Desktop\\d.zip', failed: false });
    await settle();
    expect(wrapper.get('[data-testid="diagnostics-status"]').text()).toBe('Saved to C:\\Users\\me\\Desktop\\d.zip');
    expect(wrapper.get('[data-testid="diagnostics-save"]').attributes('disabled')).toBeUndefined();
    await wrapper.get('[data-testid="diagnostics-reveal"]').trigger('click');
    await settle();
    expect(core.sent).toContainEqual({ type: 'revealDiagnostics' });
  });

  it('says nothing more after a cancelled dialog, and why after a failure', async () => {
    const { wrapper, core } = await open();
    await wrapper.get('[data-testid="diagnostics-save"]').trigger('click');
    await settle();
    core.emit({ type: 'diagnosticsSaved', failed: false });
    await settle();
    expect(wrapper.find('[data-testid="diagnostics-status"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="diagnostics-reveal"]').exists()).toBe(false);

    await wrapper.get('[data-testid="diagnostics-save"]').trigger('click');
    await settle();
    core.emit({ type: 'diagnosticsSaved', failed: true });
    await settle();
    expect(wrapper.get('[data-testid="diagnostics-status"]').text()).toBe(
      'The ZIP could not be saved; the status bar says why.',
    );
  });
});
