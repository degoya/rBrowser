import { describe, expect, it } from 'vitest';
import { createShellI18n } from './index';

describe('the shell i18n', () => {
  it('finds the toolbar texts in the catalogue or the job part', () => {
    const { t, te } = createShellI18n().global;
    expect(te('shell.toolbar.back')).toBe(true);
    expect(t('shell.toolbar.reload')).toBe('Reload all views');
    expect(t('shell.error.request_failed', { code: 501, message: 'later' })).toBe(
      'The core refused the request: later',
    );
  });
});
