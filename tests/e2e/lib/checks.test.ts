import { describe, expect, it } from 'vitest';

import { diffKeys, findingKey, parseSnapshot, viewsOf, workspaceVersion } from './checks.ts';

describe('viewsOf (K5)', () => {
  it('reads id, width and height of each breakpoint, as expected-static.json lists them', () => {
    const config = {
      version: 1,
      breakpoints: [
        { id: 'mobile', label: 'Mobile', width: 375, height: 667 },
        { id: 'sm', width: 640, height: 1138, dpr: 2 },
      ],
    };
    expect(viewsOf(config)).toEqual([
      ['mobile', 375, 667],
      ['sm', 640, 1138],
    ]);
    expect(viewsOf({})).toEqual([]);
  });
});

describe('the hreflang findings (K8)', () => {
  const snapshot = [
    'error hreflang.code_invalid hreflang.code_suggestion /hreflang-broken/en/ code=en-UK | suggestion=en-GB',
    'warning hreflang.x_default_missing hreflang.x_default_missing /hreflang-broken/en/',
    'warning hreflang.x_default_missing hreflang.x_default_missing /hreflang-broken/en/',
    '',
  ].join('\n');

  it('takes severity, rule, message and path of each snapshot line', () => {
    expect(parseSnapshot(snapshot)).toEqual([
      'error hreflang.code_invalid hreflang.code_suggestion /hreflang-broken/en/',
      'warning hreflang.x_default_missing hreflang.x_default_missing /hreflang-broken/en/',
      'warning hreflang.x_default_missing hreflang.x_default_missing /hreflang-broken/en/',
    ]);
  });

  it('keys a finding of seo_check the same way, below the fixture server origin', () => {
    const finding = {
      severity: 'error',
      rule: 'hreflang.code_invalid',
      message: 'plugin.seo.findings.hreflang.code_suggestion',
      url: 'http://127.0.0.1:18766/hreflang-broken/en/',
    };
    expect(findingKey(finding, 'http://127.0.0.1:18766')).toBe(
      'error hreflang.code_invalid hreflang.code_suggestion /hreflang-broken/en/',
    );
    expect(findingKey({ ...finding, url: undefined }, 'http://127.0.0.1:18766')).toBe(
      'error hreflang.code_invalid hreflang.code_suggestion -',
    );
  });

  it('names what is missing and what is unexpected, counting repeats', () => {
    const expected = parseSnapshot(snapshot);
    const actual = [expected[0] ?? '', expected[1] ?? '', 'info images.alt images.alt /x/'];
    expect(diffKeys(expected, actual)).toEqual({
      missing: ['warning hreflang.x_default_missing hreflang.x_default_missing /hreflang-broken/en/'],
      unexpected: ['info images.alt images.alt /x/'],
    });
    expect(diffKeys(expected, [...expected])).toEqual({ missing: [], unexpected: [] });
  });
});

describe('workspaceVersion (M12)', () => {
  it('reads the version of [workspace.package], not of a dependency', () => {
    const toml = '[workspace.dependencies]\nversion = "1.0.0"\n\n[workspace.package]\nedition = "2024"\nversion = "0.9.9-pre.2"\n';
    expect(workspaceVersion(toml)).toBe('0.9.9-pre.2');
    expect(workspaceVersion('[package]\nversion = "1"\n')).toBe('');
  });
});
