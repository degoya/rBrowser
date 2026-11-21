import { afterEach, describe, expect, it, vi } from 'vitest';
import { foldersOf, highlightOf, writeClipboard } from './code';

describe('code and export choices', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('lists every folder, the ones above included', () => {
    const listed = (id: string) => ({ id, name: '', method: 'GET', url: '' });
    expect(foldersOf([listed('me.http'), listed('shop/users/create.http'), listed('shop/media/up.http'), listed('admin/x.http')])).toEqual([
      'admin',
      'shop',
      'shop/media',
      'shop/users',
    ]);
  });

  it('colours fetch and Playwright as JavaScript', () => {
    expect(highlightOf('fetch')).toBe('js');
    expect(highlightOf('playwright')).toBe('js');
    expect(highlightOf('curl')).toBeNull();
  });

  it('copies with the async clipboard, else says it could not', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    expect(await writeClipboard('curl https://x/')).toBe(true);
    expect(writeText).toHaveBeenCalledWith('curl https://x/');
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: vi.fn().mockRejectedValue(new Error('denied')) }, configurable: true });
    Object.defineProperty(document, 'execCommand', { value: () => false, configurable: true });
    expect(await writeClipboard('x')).toBe(false);
    expect(document.querySelector('textarea')).toBeNull();
  });
});
