import { describe, expect, it } from 'vitest';
import { failureDetail, failureKey } from './failure';

describe('the start page failure', () => {
  it('turns a known code into its sentence and any other into the general one', () => {
    expect(failureKey('sitemap.error.no_project: open a project')).toBe('sitemap.start.error.no_project');
    expect(failureKey('sitemap.error.discover: plugin.sitemap-capture.errors.http_status')).toBe('sitemap.start.error.discover');
    expect(failureKey('sitemap.error.store: disk full')).toBe('sitemap.start.error.failed');
    expect(failureKey('')).toBe('sitemap.start.error.failed');
  });

  it('finds the code of a refused start behind the command failure', () => {
    const busy = 'command failed: sitemap.error.busy: a sitemap capture is already running';
    expect(failureKey(busy, 'sitemap.start.refused')).toBe('sitemap.start.error.busy');
    expect(failureDetail(busy)).toBe('a sitemap capture is already running');
    expect(failureKey('command failed: sitemap.error.foreign_url: https://x.test/ is not')).toBe('sitemap.start.error.foreign_url');
    expect(failureKey('command failed: sitemap.error.unknown_discovery: no sitemap discovery 1-1')).toBe(
      'sitemap.start.error.unknown_discovery',
    );
    expect(failureKey('command failed: timed out', 'sitemap.start.refused')).toBe('sitemap.start.refused');
  });

  it('shows the details without the code', () => {
    expect(failureDetail('sitemap.error.no_pages: the sitemap gave no page')).toBe('the sitemap gave no page');
    expect(failureDetail('timed out')).toBe('timed out');
  });
});
