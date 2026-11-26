import { describe, expect, it } from 'vitest';
import { createShellI18n } from './index';

// RB-099-22: a count in a notice is a plural, not "(s)" and not "1 ... were".
describe('the counts of the catalogue', () => {
  const { t } = createShellI18n().global;

  it('says one plugin update in the singular', () => {
    expect(t('plugins.catalog.notice.auto_updated', { count: 1 })).toBe(
      '1 plugin update was installed; it loads at the next start',
    );
    expect(t('plugins.catalog.notice.auto_updated', { count: 3 })).toBe(
      '3 plugin updates were installed; they load at the next start',
    );
    expect(t('plugins.catalog.notice.auto_updated_live', { count: 1 })).toBe(
      '1 plugin update was installed and loaded',
    );
    expect(t('plugins.catalog.notice.updates', { count: 1 })).toBe('1 plugin update in the catalog');
  });

  it('writes the screenshots and videos saved without a pseudo-plural', () => {
    expect(t('capture.notice.saved', { count: 1, folder: 'C:/shots' })).toBe('1 screenshot saved to C:/shots.');
    expect(t('video.notice.saved', { count: 2, folder: 'C:/videos' })).toBe('2 videos saved to C:/videos.');
  });

  it('lets the verb follow the regressions of a visual comparison', () => {
    expect(t('visual.notice.compared', { regressions: 1, count: 4, folder: 'diff' })).toBe(
      '1 of 4 views differs from its baseline. Diff images: diff',
    );
    expect(t('visual.notice.compared', { regressions: 0, count: 1, folder: 'diff' })).toBe(
      '0 of 1 view differ from their baselines. Diff images: diff',
    );
  });
});
