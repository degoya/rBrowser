// RB-092-03: the address field of a decoupled view's label on the canvas board. The core opens it
// (`uiCommand` `view.edit_address`, after a click on the address piece, `Leader S L` or the page
// menu); Enter sends the typed text for that view, Escape closes it.
import { afterEach, describe, expect, it } from 'vitest';
import type { VueWrapper } from '@vue/test-utils';
import type { ShellToCore } from '../ipc/protocol';
import type { MockTransport } from '../ipc/mockTransport';
import { settle } from '../test/mountShell';
import { cleanup, sentOf, setup } from '../test/canvasBoard';

afterEach(cleanup);

/** The mock core takes `view.edit_address` as the core does once the view loads the address. */
function acceptEdits(mock: MockTransport): void {
  const original = mock.send.bind(mock);
  mock.send = async (message: ShellToCore, options) => {
    if (message.type === 'runCommand' && message.id === 'view.edit_address') {
      mock.sent.push(message);
      return { type: 'ack' };
    }
    return original(message, options);
  };
}

function input(wrapper: VueWrapper, view: number) {
  const found = wrapper.get(`[data-testid="board-address-field-${view}"]`);
  return found.element.tagName === 'INPUT' ? found : found.get('input');
}

async function opened(url = 'http://localhost:3000/b') {
  const board = await setup();
  const { mock, first } = board;
  mock.state.layout.modes!.board!.items[0]!.address = { text: 'localhost:3000/b', tooltip: 'Address of Mobile' };
  mock.emit({ type: 'state', ...structuredClone(mock.state) });
  await settle();
  mock.emit({ type: 'uiCommand', id: 'view.edit_address', args: { view: first.id, url } });
  await settle();
  return board;
}

describe('the address field of a decoupled view on the canvas board', () => {
  it("opens on the core's word in that view's label, in the address's place, focused", async () => {
    const { wrapper, first, second } = await opened();
    const field = input(wrapper, first.id);
    expect((field.element as HTMLInputElement).value).toBe('http://localhost:3000/b');
    expect(document.activeElement).toBe(field.element);
    expect(wrapper.get(`label[for="${field.attributes('id')}"]`).text()).toContain(first.label);
    expect(wrapper.find(`[data-testid="board-piece-address-${first.id}"]`).exists()).toBe(false);
    expect(wrapper.find(`[data-testid="board-address-field-${second.id}"]`).exists()).toBe(false);
  });

  it('sends what was typed for that view only and closes once the core took it', async () => {
    const { wrapper, mock, first } = await opened();
    acceptEdits(mock);
    const field = input(wrapper, first.id);
    await field.setValue('/pricing');
    await wrapper.get(`[data-testid="board-address-form-${first.id}"]`).trigger('submit');
    await settle();
    expect(sentOf(mock, 'runCommand')).toEqual([
      { type: 'runCommand', id: 'view.edit_address', args: { view: first.id, url: '/pricing' } },
    ]);
    expect(wrapper.find(`[data-testid="board-address-field-${first.id}"]`).exists()).toBe(false);
    expect(wrapper.find(`[data-testid="board-piece-address-${first.id}"]`).exists()).toBe(true);
    expect(sentOf(mock, 'canvasDrag')).toEqual([]);
  });

  it('stays open and marked when the core refuses, the reason being the core\'s to tell', async () => {
    const { wrapper, mock, first } = await opened();
    const field = input(wrapper, first.id);
    await field.setValue('javascript:alert(1)');
    await wrapper.get(`[data-testid="board-address-form-${first.id}"]`).trigger('submit');
    await settle();
    expect(sentOf(mock, 'runCommand')).toHaveLength(1);
    expect(input(wrapper, first.id).attributes('aria-invalid')).toBe('true');
    expect(mock.reports, 'the core names the reason in the status bar').toEqual([]);
  });

  it('closes on Escape and sends nothing', async () => {
    const { wrapper, mock, first } = await opened();
    await input(wrapper, first.id).trigger('keydown', { key: 'Escape' });
    await settle();
    expect(wrapper.find(`[data-testid="board-address-field-${first.id}"]`).exists()).toBe(false);
    expect(sentOf(mock, 'runCommand')).toEqual([]);
    expect(sentOf(mock, 'canvasStep')).toEqual([]);
  });
});
