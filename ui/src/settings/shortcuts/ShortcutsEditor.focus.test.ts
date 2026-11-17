// Where the focus goes when a control of the shortcut editor goes away or disables itself
// (RB-090-23): back to the control that started it, never to the body.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import { createFakeCore } from '../test/fakeCore';
import { mountPage, settle } from '../test/mountPage';
import ShortcutsEditor from './ShortcutsEditor.vue';

let current: VueWrapper | null = null;

async function open() {
  const mounted = await mountPage(ShortcutsEditor, {}, createFakeCore());
  current = mounted.wrapper;
  return mounted;
}

afterEach(() => {
  current?.unmount();
  current = null;
});

const ROW = '[data-command="reload.all"]';

async function startRecording(wrapper: VueWrapper) {
  await wrapper.get(`${ROW} [data-testid="record"]`).trigger('click');
  await settle();
  return wrapper.get(`${ROW} [data-testid="recorder"]`);
}

function recordButton(wrapper: VueWrapper): Element {
  return wrapper.get(`${ROW} [data-testid="record"]`).element;
}

describe('the focus in the shortcut editor', () => {
  it('names the command on every Record button, the visible word first', async () => {
    const { wrapper } = await open();
    const button = wrapper.get(`${ROW} [data-testid="record"]`);
    const title = wrapper.get(`${ROW} [data-testid="command-title"]`).text();
    expect(button.text()).toBe('Record');
    expect(button.attributes('aria-label')).toBe(`Record shortcut for ${title}`);
  });

  it('goes back to Record when Escape ends the recording', async () => {
    const { wrapper } = await open();
    const recorder = await startRecording(wrapper);
    expect(document.activeElement).toBe(recorder.element);
    await recorder.trigger('keydown', { code: 'Escape' });
    await settle();
    expect(document.activeElement).toBe(recordButton(wrapper));
  });

  it('goes back to Record when a binding is recorded', async () => {
    const { wrapper } = await open();
    const recorder = await startRecording(wrapper);
    await recorder.trigger('keydown', { code: 'F9', ctrlKey: true, shiftKey: true });
    await settle();
    expect(document.activeElement).toBe(recordButton(wrapper));
  });

  it('goes back to Record when the conflict dialog closes', async () => {
    const { wrapper } = await open();
    const recorder = await startRecording(wrapper);
    for (const press of [{ code: 'Space', ctrlKey: true, shiftKey: true }, { code: 'KeyS' }, { code: 'KeyS' }]) {
      await recorder.trigger('keydown', press);
    }
    await settle();
    await wrapper.get(`${ROW} [data-testid="conflict-cancel"]`).trigger('click');
    await settle();
    expect(wrapper.find(`${ROW} [data-testid="conflict"]`).exists()).toBe(false);
    expect(document.activeElement).toBe(recordButton(wrapper));
  });

  it('goes to Record when the removed binding takes its button along', async () => {
    const { wrapper } = await open();
    const remove = wrapper.get(`${ROW} [aria-label="Remove Ctrl+Shift+Space R R"]`);
    (remove.element as HTMLElement).focus();
    await remove.trigger('click');
    await settle();
    expect(wrapper.find(`${ROW} [aria-label="Remove Ctrl+Shift+Space R R"]`).exists()).toBe(false);
    expect(document.activeElement).toBe(recordButton(wrapper));
  });

  it('goes to Record when Reset disables itself', async () => {
    const { wrapper } = await open();
    const recorder = await startRecording(wrapper);
    await recorder.trigger('keydown', { code: 'F9', ctrlKey: true, shiftKey: true });
    await settle();
    const reset = wrapper.get(`${ROW} [data-testid="reset"]`);
    expect(reset.attributes('disabled')).toBeUndefined();
    (reset.element as HTMLElement).focus();
    await reset.trigger('click');
    await settle();
    expect(reset.attributes('disabled')).toBeDefined();
    expect(document.activeElement).toBe(recordButton(wrapper));
  });

  it('goes back to "Search by keys" when the key search ends or is cleared', async () => {
    const { wrapper } = await open();
    const search = () => wrapper.get('[data-testid="key-search"]').element;
    await wrapper.get('[data-testid="key-search"]').trigger('click');
    await settle();
    await wrapper.get('[data-testid="key-search-capture"]').trigger('keydown', { code: 'Escape' });
    await settle();
    expect(document.activeElement).toBe(search());

    await wrapper.get('[data-testid="key-search"]').trigger('click');
    await settle();
    await wrapper
      .get('[data-testid="key-search-capture"]')
      .trigger('keydown', { code: 'Digit2', ctrlKey: true, shiftKey: true });
    await settle();
    expect(document.activeElement).toBe(search());

    const clear = wrapper.get('[data-testid="key-search-clear"]');
    (clear.element as HTMLElement).focus();
    await clear.trigger('click');
    await settle();
    expect(document.activeElement).toBe(search());
  });
});
