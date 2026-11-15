// The toolbar's overflow menu (RB-080-52): browsing with the keyboard runs nothing, the entries
// are the ones shown, a control that opens something gets its group back, notices are marked.
import { describe, expect, it, vi } from 'vitest';
import { EVENT_NAME } from '../ipc/protocol';
import type { sampleState } from '../ipc/mockState';
import { settle } from '../test/mountShell';
import { menuLabels, resize, rowWidth, setup, states, useToolbarLayout } from '../test/toolbarLayout';

useToolbarLayout();

describe('the overflow menu', () => {
  const command = () =>
    window.dispatchEvent(
      new CustomEvent(EVENT_NAME, { detail: { type: 'uiCommand', id: 'ui.toolbar.overflow.open' } }),
    );
  /** What a closed select does on Windows for an arrow key: the next option, and `change`. */
  const walk = async (select: HTMLSelectElement, key: string, value: string) => {
    const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
    select.dispatchEvent(down);
    if (!down.defaultPrevented) {
      select.value = value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }
    await settle();
    select.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
    return down;
  };

  it('runs nothing while the keyboard browses the closed menu, only what is chosen', async () => {
    const { wrapper, mock } = await setup(500);
    command();
    await settle();
    const select = wrapper.get('[data-testid="toolbar-overflow-select"]');
    const element = select.element as HTMLSelectElement;
    expect(document.activeElement).toBe(element);
    const sync = select.findAll('option').find((option) => option.text() === 'Sync: on');
    const value = sync?.attributes('value') ?? '';
    expect(value).not.toBe('');
    const before = mock.sent.length;
    // No popup here (no `showPicker`): arrows and a typed letter walk nothing and run nothing.
    for (const key of ['ArrowDown', 'ArrowDown', 'ArrowUp', 's']) {
      const down = await walk(element, key, value);
      expect(down.defaultPrevented, key).toBe(true);
    }
    // Even a `change` such a key causes all the same chooses nothing.
    element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    element.value = value;
    element.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
    expect(mock.sent).toHaveLength(before);
    expect(element.value).toBe('');
    element.dispatchEvent(new KeyboardEvent('keyup', { key: 'ArrowDown', bubbles: true }));
    // An explicit choice runs.
    await select.setValue(value);
    await settle();
    expect(mock.sent.at(-1)).toEqual({ type: 'setSync', feature: 'all', enabled: false });
  });

  it('opens its list on the first key, which is the gesture the command lacks', async () => {
    const { wrapper, mock } = await setup(500);
    const element = wrapper.get('[data-testid="toolbar-overflow-select"]').element as HTMLSelectElement;
    const showPicker = vi.fn();
    element.showPicker = showPicker;
    const before = mock.sent.length;
    for (const key of ['ArrowDown', 'Enter', ' ']) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      expect(down.defaultPrevented, key).toBe(true);
    }
    expect(showPicker).toHaveBeenCalledTimes(3);
    // A modifier or Tab is no reason to open.
    for (const key of ['Shift', 'Tab', 'Escape']) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
    expect(showPicker).toHaveBeenCalledTimes(3);
    expect(mock.sent).toHaveLength(before);
  });

  it('keeps the entries it showed while the window state changes under the open list', async () => {
    const { wrapper, mock } = await setup(500);
    const select = wrapper.get('[data-testid="toolbar-overflow-select"]');
    await select.trigger('pointerdown');
    const shown = select.findAll('option').map((option) => [option.attributes('value'), option.text()]);
    mock.state.bookmarks = [];
    mock.state.sync.enabled = false;
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(select.findAll('option').map((option) => [option.attributes('value'), option.text()])).toEqual(shown);
    // The next press reads them anew.
    expect(await menuLabels(wrapper)).toContain('Sync: off');
  });

  /** The emulation extras: a popover drawn in the page beside its button (`aria-expanded`). */
  const EXTRAS = 'Location, time zone, CPU, offline and print';

  it('brings a group back into the row when its control opens something beside itself', async () => {
    const { wrapper } = await setup(800);
    const select = wrapper.get('[data-testid="toolbar-overflow-select"]');
    await select.trigger('pointerdown');
    const extras = select.findAll('option').find((option) => option.text() === EXTRAS);
    expect(extras).toBeDefined();
    expect(extras?.attributes('disabled')).toBeUndefined();
    expect(wrapper.get('[data-testid="emulation-extras"]').attributes('aria-expanded')).toBe('false');
    expect(states(wrapper).emulation).toBe('hidden');
    await select.setValue(extras?.attributes('value'));
    await settle();
    // In the row again, so what it opens is seen; the address bar keeps its floor for it.
    expect(states(wrapper).emulation).toBe('shown');
    expect(wrapper.get('[data-testid="emulation-extras"]').attributes('aria-expanded')).toBe('true');
    expect(rowWidth(wrapper)).toBeLessThanOrEqual(800);
    expect(wrapper.get('form').attributes('style') ?? '').not.toContain('min-width');
    // The person goes on: the menu again, and the group leaves the row.
    await wrapper.get('[data-testid="toolbar-overflow-select"]').trigger('pointerdown');
    await settle();
    expect(states(wrapper).emulation).toBe('hidden');
  });

  it('says so where the row has no room for the group beside the address bar', async () => {
    const { wrapper, mock } = await setup(500);
    const select = wrapper.get('[data-testid="toolbar-overflow-select"]');
    await select.trigger('pointerdown');
    const extras = select.findAll('option').find((option) => option.text().startsWith(EXTRAS));
    expect(extras?.text()).toBe(`${EXTRAS} (needs a wider window)`);
    expect(extras?.attributes('disabled')).toBeDefined();
    const before = mock.sent.length;
    await select.setValue(extras?.attributes('value'));
    await settle();
    expect(states(wrapper).emulation).toBe('hidden');
    expect(wrapper.get('[data-testid="emulation-extras"]').attributes('aria-expanded')).toBe('false');
    expect(mock.sent).toHaveLength(before);
  });

  it('just clicks a control whose menu the core draws: it needs no room in the row', async () => {
    const { wrapper, mock } = await setup(500);
    const select = wrapper.get('[data-testid="toolbar-overflow-select"]');
    await select.trigger('pointerdown');
    // The "+" asks for a native menu under itself (`aria-haspopup`, RB-080-46).
    const add = select.findAll('option').find((option) => option.text() === 'Add view');
    expect(add?.attributes('disabled')).toBeUndefined();
    await select.setValue(add?.attributes('value'));
    await settle();
    expect(mock.sent.at(-1)).toMatchObject({ type: 'addViewMenu' });
    expect(states(wrapper).layout).toBe('hidden');
  });

  it('marks the button and names the notice when a hidden group tells something', async () => {
    const paused = (state: ReturnType<typeof sampleState>) => {
      state.sync.enabled = false;
      state.sync.browsersync = 'http://shop.test:3000';
    };
    const { wrapper } = await setup(500, {}, paused);
    expect(states(wrapper).browsersync).toBe('hidden');
    expect(wrapper.find('[data-testid="toolbar-overflow-mark"]').exists()).toBe(true);
    const select = wrapper.get('[data-testid="toolbar-overflow-select"]');
    expect(select.attributes('title')).toBe('More (with notices)');
    await select.trigger('pointerdown');
    const options = select.findAll('option');
    const notice = options.findIndex((option) => option.text().includes('shop.test'));
    expect(notice).toBeGreaterThan(0);
    expect(options[notice]?.attributes('disabled')).toBeDefined();
    // The button that answers it follows.
    expect(options[notice + 1]?.attributes('disabled')).toBeUndefined();
    // Without a notice, no mark.
    await resize(2400);
    expect(wrapper.find('[data-testid="toolbar-overflow-mark"]').exists()).toBe(false);
  });

  it('shows no mark for plain controls', async () => {
    const { wrapper } = await setup(500);
    expect(wrapper.find('[data-testid="toolbar-overflow-mark"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="toolbar-overflow-select"]').attributes('title')).toBe('More');
  });
});
