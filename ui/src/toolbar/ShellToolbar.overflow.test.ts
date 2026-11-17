// The toolbar in a narrow window (RB-080-52): what stays, what shrinks, what leaves the row, and
// the second row of `ui.toolbar.overflow = wrap`. The widths are `test/toolbarLayout.ts`.
import { describe, expect, it } from 'vitest';
import { EVENT_NAME } from '../ipc/protocol';
import { settle } from '../test/mountShell';
import { lastNativeMenu } from '../test/nativeMenu';
import { chooseInMenu, menuLabels, menuRows, resize, rowWidth, setup, states, useToolbarLayout } from '../test/toolbarLayout';
import { GAP, ICON_WIDTH, PADDING } from './fit';

useToolbarLayout();

/** The row that holds every group but the canvas minimap at the end, with the main menu. */
const WIDE_ENOUGH_BUT_THE_MINIMAP = 1440;

describe('the toolbar in a narrow window', () => {
  // RB-090-63: the main menu `≡` is always in the row.
  it('leaves a wide window as it was: every group in the row, the main menu at its end', async () => {
    const { wrapper } = await setup(2400);
    expect(Object.values(states(wrapper)).every((state) => state === 'shown')).toBe(true);
    expect(wrapper.find('[data-testid="toolbar-overflow"]').exists()).toBe(true);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('1');
  });

  it.each([1800, 1400, 1100, 800])(
    'at %i px: the address bar keeps its width, nothing is cut off, the rest is in the menu',
    async (width) => {
      const { wrapper } = await setup(width);
      // The address bar: there, usable, never under its floor.
      const form = wrapper.get('form');
      expect(form.classes()).toContain('min-w-60');
      expect(form.element.closest('[data-state="hidden"]')).toBeNull();
      expect(wrapper.get('input[type="text"]').attributes('disabled')).toBeUndefined();
      // Nothing is cut off: the row with the address bar at its floor is no wider than the window.
      expect(rowWidth(wrapper)).toBeLessThanOrEqual(width);
      // Navigation stays.
      const now = states(wrapper);
      expect(now.navigation).toBe('shown');
      // What left the row is in the menu, by name.
      const hidden = Object.keys(now).filter((id) => now[id] === 'hidden' && id !== 'logo');
      expect(wrapper.find('[data-testid="toolbar-overflow"]').exists()).toBe(true);
      const labels = await menuLabels(wrapper);
      for (const id of hidden) {
        const group = wrapper.get(`[data-toolbar-group="${id}"]`);
        expect(group.attributes('aria-hidden')).toBe('true');
        for (const button of group.findAll('button')) {
          const name = button.attributes('aria-label') ?? '';
          expect(labels.some((label) => label.includes(name)), `${id}: ${name}`).toBe(true);
        }
      }
    },
  );

  it('shrinks the wide pulldowns to icons before anything leaves the row', async () => {
    const { wrapper } = await setup(1600);
    const now = states(wrapper);
    expect(now.bookmarks).toBe('compact');
    expect(now.environment).toBe('compact');
    expect(now.layout).toBe('compact');
    expect(Object.values(now)).not.toContain('hidden');
    // Each pulldown is its group's icon alone (RB-090-45), still the control, its name the tooltip.
    const pulldowns = wrapper.findAll('[data-pulldown]');
    expect(pulldowns).toHaveLength(3);
    for (const pulldown of pulldowns) {
      expect(pulldown.classes()).toContain('size-8');
      expect(pulldown.text()).toBe('');
    }
    expect(wrapper.get('[data-toolbar-group="environment"] [data-pulldown]').attributes('title')).toBe(
      'Environment: local',
    );
  });

  it('sends the canvas minimap into the menu first', async () => {
    // The groups of `test/toolbarLayout.ts` with the main menu always in the row (RB-090-63).
    const { wrapper } = await setup(WIDE_ENOUGH_BUT_THE_MINIMAP);
    const now = states(wrapper);
    expect(Object.keys(now).filter((id) => now[id] === 'hidden')).toEqual(['minimap-end']);
    // On top of the main menu, before its own rows.
    expect((await menuLabels(wrapper))[0]).toBe('Canvas minimap');
  });

  it('keeps navigation, address bar and menu below the smallest sensible width', async () => {
    const { wrapper } = await setup(420);
    const now = states(wrapper);
    expect(now.navigation).toBe('shown');
    expect(now.logo).toBe('hidden');
    expect(Object.entries(now).filter(([, state]) => state !== 'hidden')).toEqual([['navigation', 'shown']]);
    expect(wrapper.find('[data-testid="toolbar-overflow"]').exists()).toBe(true);
    expect(rowWidth(wrapper)).toBeLessThanOrEqual(420);
  });

  it('runs the commands of what left the row from the menu', async () => {
    const { wrapper, mock } = await setup(500);
    const choose = async (label: string) => {
      expect(await chooseInMenu(wrapper, label), label).toBeDefined();
    };
    await choose('Settings');
    expect(mock.sent.at(-1)).toEqual({ type: 'openSettings' });
    // A hidden pulldown's rows stand under its name; choosing one runs its command.
    expect(await menuRows(wrapper)).toContainEqual({ kind: 'heading', label: 'Environment: local' });
    await choose('staging');
    expect(mock.sent.at(-1)).toEqual({ type: 'selectEnvironment', id: 'staging' });
    // A bookmark under its group's name.
    await choose('Shop / Checkout - /checkout');
    expect(mock.sent.at(-1)).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'checkout' } });
    // A switch is a check row (RB-090-63).
    expect((await menuRows(wrapper)).find((row) => row.kind === 'item' && row.label === 'Sync')).toMatchObject({
      checked: true,
    });
    await choose('Sync');
    expect(mock.sent.at(-1)).toEqual({ type: 'setSync', feature: 'all', enabled: false });
    await settle();
    expect((await menuRows(wrapper)).find((row) => row.kind === 'item' && row.label === 'Sync')).toMatchObject({
      checked: false,
    });
  });

  it('follows the window: what fits again comes back', async () => {
    const { wrapper } = await setup(800);
    expect(Object.values(states(wrapper))).toContain('hidden');
    await resize(2400);
    expect(Object.values(states(wrapper)).every((state) => state === 'shown')).toBe(true);
  });

  it('opens the main menu on ui.toolbar.overflow.open, also when nothing left the row', async () => {
    const { wrapper, mock } = await setup(800);
    // As the core's events reach the page.
    const command = () =>
      window.dispatchEvent(
        new CustomEvent(EVENT_NAME, { detail: { type: 'uiCommand', id: 'ui.toolbar.overflow.open' } }),
      );
    command();
    await settle();
    expect(lastNativeMenu(mock.sent).menu).toMatch(/^menu-\d+$/);
    expect(document.activeElement).toBe(wrapper.get('[data-testid="toolbar-overflow-button"]').element);
    await resize(2400);
    const before = mock.sent.filter((message) => message.type === 'nativeMenu').length;
    command();
    await settle();
    expect(mock.sent.filter((message) => message.type === 'nativeMenu')).toHaveLength(before + 1);
  });
});

describe('ui.toolbar.overflow = wrap', () => {
  const wrap = { 'ui.toolbar.overflow': 'wrap' };

  it('asks the core for a second row and wraps once it has it', async () => {
    // The second row holds every group and the main menu (RB-090-63).
    const width = 1536;
    const { wrapper, mock } = await setup(width, wrap);
    expect(mock.sent).toContainEqual({ type: 'toolbarRows', rows: 2 });
    const toolbar = wrapper.get('[role="toolbar"]');
    expect(toolbar.attributes('data-rows')).toBe('2');
    expect(toolbar.classes()).toContain('flex-wrap');
    // Everything fits into two rows: nothing shrinks, nothing leaves; the main menu stays.
    expect(Object.values(states(wrapper)).every((state) => state === 'shown')).toBe(true);
    // The address bar ends the first row: logo, navigation and it, nothing beside them.
    expect(wrapper.get('form').attributes('style')).toContain(`${width - PADDING - 96 - 88 - 2 * GAP - 4}px`);
  });

  it('stays in one row while everything fits, and goes back to one', async () => {
    const { wrapper, mock } = await setup(2400, wrap);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('1');
    expect(mock.sent).not.toContainEqual({ type: 'toolbarRows', rows: 2 });
    await resize(1100);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('2');
    await resize(2400);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('1');
    expect(mock.sent.at(-1)).toEqual({ type: 'toolbarRows', rows: 1 });
    expect(wrapper.get('form').attributes('style') ?? '').not.toContain('px');
  });

  it('uses the menu for what a second row does not hold either', async () => {
    const { wrapper } = await setup(700, wrap);
    const now = states(wrapper);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('2');
    expect(now.navigation).toBe('shown');
    expect(now['minimap-end']).toBe('hidden');
    expect(wrapper.find('[data-testid="toolbar-overflow"]').exists()).toBe(true);
  });

  it('never wraps while the setting is menu', async () => {
    const { wrapper, mock } = await setup(1100);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('1');
    expect(mock.sent).not.toContainEqual({ type: 'toolbarRows', rows: 2 });
  });

  it('asks again after a refusal, without a word in the status bar', async () => {
    const { wrapper, mock } = await setup(2400, wrap);
    const send = mock.send.bind(mock);
    let refuse = true;
    mock.send = (message) => {
      if (message.type === 'toolbarRows' && message.rows === 2 && refuse) {
        refuse = false;
        mock.sent.push(message);
        return Promise.reject(new Error('the toolbar has 1 to 1 rows, not 2'));
      }
      return send(message);
    };
    const reports = mock.reports.length;
    await resize(1300);
    // Refused: one row, held together by the menu.
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('1');
    expect(mock.reports).toHaveLength(reports);
    // The next fit asks again.
    await resize(1299);
    expect(mock.sent.filter((m) => m.type === 'toolbarRows' && m.rows === 2)).toHaveLength(2);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('2');
    expect(mock.reports).toHaveLength(reports);
  });

  it('keeps the address bar its 240 px where navigation has no room beside it', async () => {
    // Navigation 88 + 240 + gap + padding = 352: narrower than that.
    const { wrapper } = await setup(330, wrap);
    const toolbar = wrapper.get('[role="toolbar"]');
    expect(toolbar.attributes('data-rows')).toBe('2');
    const form = wrapper.get('form');
    // The floor stands, the address bar has the first row alone ...
    expect(form.classes()).toContain('min-w-60');
    const style = form.attributes('style') ?? '';
    expect(style).not.toContain('min-width');
    expect(style).toContain('100%');
    expect(style).toMatch(/order:\s*-1/);
    // ... and navigation leads the second, with the menu for the rest.
    const now = states(wrapper);
    expect(now.navigation).toBe('shown');
    expect(now.logo).toBe('hidden');
    expect(wrapper.find('[data-testid="toolbar-overflow"]').exists()).toBe(true);
    const second = wrapper
      .findAll('[data-toolbar-group]')
      .filter((group) => group.attributes('data-state') !== 'hidden')
      .map((group) => (group.element as HTMLElement).offsetWidth)
      .filter((width) => width > 0);
    const used = PADDING + [...second, ICON_WIDTH].reduce((sum, width) => sum + width + GAP, 0) - GAP;
    expect(used).toBeLessThanOrEqual(330);
  });

  it.each([1100, 800, 500, 400, 330])('never drops the floor in wrap mode at %i px', async (width) => {
    const { wrapper } = await setup(width, wrap);
    expect(wrapper.get('[role="toolbar"]').attributes('data-rows')).toBe('2');
    expect(wrapper.get('form').attributes('style') ?? '').not.toContain('min-width');
    expect(wrapper.get('form').attributes('style') ?? '').not.toContain(' 0px');
  });
});
