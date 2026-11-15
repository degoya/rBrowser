import { describe, expect, it, vi } from 'vitest';
import type { ShellToCore } from '../ipc/protocol';
import { createLinkOpener } from './opener';
import type { OpenLinksIn } from './open';

function opener(mode: OpenLinksIn) {
  const sent: ShellToCore[] = [];
  const send = vi.fn(async (message: ShellToCore) => {
    sent.push(message);
    return true;
  });
  return { sent, opener: createLinkOpener(send, () => mode) };
}

describe('the link opener', () => {
  it('opens at once where the setting says', async () => {
    const { sent, opener: links } = opener('system');
    await expect(links.open('https://help.test/rule')).resolves.toBe(true);
    expect(sent).toEqual([{ type: 'runCommand', id: 'app.open_link', args: { url: 'https://help.test/rule' } }]);
    expect(links.asking.value).toBeNull();
  });

  it('asks first for ask, and sends the place chosen', async () => {
    const { sent, opener: links } = opener('ask');
    const opened = links.open('https://help.test/rule');
    expect(links.asking.value).toBe('https://help.test/rule');
    expect(sent).toEqual([]);
    links.answer('activeView');
    await expect(opened).resolves.toBe(true);
    expect(sent).toEqual([
      { type: 'runCommand', id: 'app.open_link', args: { url: 'https://help.test/rule', where: 'activeView' } },
    ]);
  });

  it('sends nothing when the question is cancelled or the address is none', async () => {
    const { sent, opener: links } = opener('ask');
    const first = links.open('https://help.test/one');
    const second = links.open('https://help.test/two');
    await expect(first).resolves.toBe(false);
    expect(links.asking.value).toBe('https://help.test/two');
    links.answer(null);
    await expect(second).resolves.toBe(false);
    await expect(links.open('javascript:alert(1)')).resolves.toBe(false);
    expect(sent).toEqual([]);
  });
});
