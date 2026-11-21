import { describe, expect, it } from 'vitest';
import type { JsonValue } from '../ipc/protocol';
import { EDIT_ADDRESS, addressEditOf, submitMessage } from './addressEdit';

// RB-092-03: the core opens the address field of a decoupled view's label on the canvas board
// with `uiCommand` `view.edit_address`; Enter sends the text back as the command's `url`.
describe('the address field of a decoupled view on the board', () => {
  it('takes the view and its address from the core', () => {
    expect(addressEditOf({ view: 3, url: 'http://localhost:5173/a' })).toEqual({
      view: 3,
      url: 'http://localhost:5173/a',
    });
    expect(addressEditOf({ view: 3 }), 'a view that loaded nothing yet').toEqual({ view: 3, url: '' });
  });

  it('opens nothing for arguments that name no view', () => {
    const cases: (JsonValue | undefined)[] = [undefined, null, 'x', [], {}, { view: '3' }, { view: -1 }, { view: 1.5 }];
    for (const args of cases) {
      expect(addressEditOf(args), JSON.stringify(args)).toBeNull();
    }
  });

  it('sends the typed text unchanged: the core checks and completes it', () => {
    expect(submitMessage(3, ' /pricing ')).toEqual({
      type: 'runCommand',
      id: EDIT_ADDRESS,
      args: { view: 3, url: ' /pricing ' },
    });
    expect(EDIT_ADDRESS).toBe('view.edit_address');
  });
});
