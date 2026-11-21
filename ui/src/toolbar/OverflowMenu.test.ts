// The toolbar's main menu `≡` (RB-090-63, the overflow menu of RB-080-52): the core's native menu
// below its button (RB-090-45); on top what left the row, the entries the ones shown, a hidden
// pulldown's rows under its name with the one in effect checked, a switch as a check row, a control
// that opens something gets its group back, notices are marked; below them the areas' submenus.
import { describe, expect, it } from 'vitest';
import { EVENT_NAME } from '../ipc/protocol';
import type { sampleState } from '../ipc/mockState';
import { settle } from '../test/mountShell';
import { closeNativeMenu, lastNativeMenu } from '../test/nativeMenu';
import { chooseInMenu, menuLabels, menuRows, resize, rowWidth, setup, states, useToolbarLayout } from '../test/toolbarLayout';

useToolbarLayout();

const BUTTON = '[data-testid="toolbar-overflow-button"]';

describe('the main menu', () => {
  const command = () =>
    window.dispatchEvent(
      new CustomEvent(EVENT_NAME, { detail: { type: 'uiCommand', id: 'ui.toolbar.overflow.open' } }),
    );

  it('is a button whose menu the core draws below it, no select', async () => {
    const { wrapper, mock } = await setup(500);
    const button = wrapper.get(BUTTON);
    expect(button.element.tagName).toBe('BUTTON');
    expect(button.attributes('aria-haspopup')).toBe('menu');
    expect(wrapper.find('[data-testid="toolbar-overflow"] select').exists()).toBe(false);
    await button.trigger('click');
    await settle();
    expect(lastNativeMenu(mock.sent).menu).toMatch(/^menu-\d+$/);
    await closeNativeMenu(mock.sent);
    expect(document.activeElement).toBe(button.element);
  });

  // RB-090-41, RB-090-45: a pulldown out of the row is a headed block of the menu, the value in
  // effect checked as in its own menu.
  it("lists a hidden pulldown's rows under its name, the one in effect checked", async () => {
    const { wrapper } = await setup(500);
    expect(states(wrapper).environment).toBe('hidden');
    const rows = await menuRows(wrapper);
    const after = (heading: string) => {
      const start = rows.findIndex((row) => row.kind === 'heading' && row.label === heading);
      expect(start, heading).toBeGreaterThanOrEqual(0);
      const end = rows.findIndex((row, index) => index > start && row.kind !== 'item');
      return rows.slice(start + 1, end < 0 ? undefined : end);
    };
    expect(after('Environment: local')).toEqual([
      expect.objectContaining({ label: 'local', checked: true }),
      expect.objectContaining({ label: 'staging', checked: false }),
    ]);
    // A line between two blocks, never two lines or one at an end.
    const kinds = rows.map((row) => row.kind).join(' ');
    expect(kinds).not.toMatch(/separator separator|^separator|separator$/);
  });

  it('runs only the row chosen; Esc runs nothing', async () => {
    const { wrapper, mock } = await setup(500);
    await wrapper.get(BUTTON).trigger('click');
    await settle();
    const before = mock.sent.length;
    await closeNativeMenu(mock.sent);
    expect(mock.sent).toHaveLength(before);
    const row = await chooseInMenu(wrapper, 'Sync');
    expect(row).toMatchObject({ checked: true });
    expect(mock.sent.at(-1)).toEqual({ type: 'setSync', feature: 'all', enabled: false });
  });

  it('opens on an arrow key or F4 and on ui.toolbar.overflow.open, the focus on its button', async () => {
    const { wrapper, mock } = await setup(500);
    const element = wrapper.get(BUTTON).element as HTMLButtonElement;
    const menus = () => mock.sent.filter((message) => message.type === 'nativeMenu').length;
    for (const key of ['Shift', 'Tab', 'Escape', 's']) {
      element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
    await settle();
    expect(menus()).toBe(0);
    for (const key of ['ArrowDown', 'F4']) {
      const down = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
      element.dispatchEvent(down);
      await settle();
      expect(down.defaultPrevented, key).toBe(true);
      await closeNativeMenu(mock.sent);
    }
    expect(menus()).toBe(2);
    element.blur();
    command();
    await settle();
    expect(menus()).toBe(3);
    expect(document.activeElement).toBe(element);
  });

  it('runs the entry it showed, though the window state changed under the open menu', async () => {
    const { wrapper, mock } = await setup(500);
    await wrapper.get(BUTTON).trigger('click');
    await settle();
    const sync = lastNativeMenu(mock.sent).rows.find((row) => row.kind === 'item' && row.label === 'Sync');
    mock.state.bookmarks = [];
    mock.state.sync.enabled = false;
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    await closeNativeMenu(mock.sent, sync?.kind === 'item' ? sync.id : undefined);
    // The switch as it is now: the click turns sync on again.
    expect(mock.sent.at(-1)).toEqual({ type: 'setSync', feature: 'all', enabled: true });
    // The next click reads them anew.
    expect(await menuLabels(wrapper)).toContain('Sync');
  });

  /** The emulation extras: a popover drawn in the page beside its button (`aria-expanded`). */
  const EXTRAS = 'Location, time zone, CPU, offline and print';

  it('brings a group back into the row when its control opens something beside itself', async () => {
    const { wrapper } = await setup(800);
    expect(wrapper.get('[data-testid="emulation-extras"]').attributes('aria-expanded')).toBe('false');
    expect(states(wrapper).emulation).toBe('hidden');
    const extras = await chooseInMenu(wrapper, EXTRAS);
    expect(extras?.kind === 'item' && extras.disabled).toBe(false);
    await settle();
    // In the row again, so what it opens is seen; the address bar keeps its floor for it.
    expect(states(wrapper).emulation).toBe('shown');
    expect(wrapper.get('[data-testid="emulation-extras"]').attributes('aria-expanded')).toBe('true');
    expect(rowWidth(wrapper)).toBeLessThanOrEqual(800);
    expect(wrapper.get('form').attributes('style') ?? '').not.toContain('min-width');
    // The person goes on: the menu again, and the group leaves the row.
    await menuRows(wrapper);
    await settle();
    expect(states(wrapper).emulation).toBe('hidden');
  });

  it('says so where the row has no room for the group beside the address bar', async () => {
    const { wrapper, mock } = await setup(500);
    const rows = await menuRows(wrapper);
    const extras = rows.find((row) => row.kind === 'item' && row.label.startsWith(EXTRAS));
    expect(extras).toMatchObject({ label: `${EXTRAS} (needs a wider window)`, disabled: true });
    // Were it told all the same, nothing would run.
    await wrapper.get(BUTTON).trigger('click');
    await settle();
    const before = mock.sent.length;
    await closeNativeMenu(mock.sent, extras?.kind === 'item' ? extras.id : undefined);
    expect(states(wrapper).emulation).toBe('hidden');
    expect(wrapper.get('[data-testid="emulation-extras"]').attributes('aria-expanded')).toBe('false');
    expect(mock.sent).toHaveLength(before);
  });

  it('just clicks a control whose menu the core draws: it needs no room in the row', async () => {
    const { wrapper, mock } = await setup(500);
    // The "+" asks for a native menu under itself (`aria-haspopup`, RB-080-46), its rows the
    // shell's since RB-094-20.
    const add = await chooseInMenu(wrapper, 'Add view');
    expect(add?.kind === 'item' && add.disabled).toBe(false);
    expect(mock.sent.at(-1)).toMatchObject({ type: 'nativeMenu', menu: expect.stringMatching(/^add-\d+$/) });
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
    expect(wrapper.get(BUTTON).attributes('title')).toBe('Main menu (with notices)');
    const rows = (await menuRows(wrapper)).filter((row) => row.kind === 'item');
    const notice = rows.findIndex((row) => row.label.includes('shop.test'));
    expect(notice).toBeGreaterThanOrEqual(0);
    expect(rows[notice]).toMatchObject({ disabled: true });
    // The button that answers it follows.
    expect(rows[notice + 1]).toMatchObject({ disabled: false });
    // Without a notice, no mark.
    await resize(2400);
    expect(wrapper.find('[data-testid="toolbar-overflow-mark"]').exists()).toBe(false);
  });

  it('shows no mark for plain controls', async () => {
    const { wrapper } = await setup(500);
    expect(wrapper.find('[data-testid="toolbar-overflow-mark"]').exists()).toBe(false);
    expect(wrapper.get(BUTTON).attributes('title')).toBe('Main menu');
  });
});
