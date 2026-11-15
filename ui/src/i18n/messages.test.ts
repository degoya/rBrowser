import { describe, expect, it } from 'vitest';
import { mergeCatalogues } from './messages';

describe('the message catalogue', () => {
  it('lays the parts under the catalogue, which wins', () => {
    const merged = mergeCatalogues({ shell: { toolbar: { back: 'Back' } } }, [
      { shell: { toolbar: { back: 'Old back', forward: 'Forward' } } },
      { settings: { title: 'Settings' } },
    ]);
    expect(merged).toEqual({
      shell: { toolbar: { back: 'Back', forward: 'Forward' } },
      settings: { title: 'Settings' },
    });
  });

  it('ignores a part that is not an object', () => {
    expect(mergeCatalogues({}, [[], 'text', null])).toEqual({});
  });
});
