// The security header panel's test fixtures (RB-093-03): the English texts by full key and an
// answer of `compare` with two environments read, one that failed and a cookie row.
import en from '../../../locales/en.json';
import type { Comparison } from './model';

function flatten(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
export const STRINGS: Record<string, string> = {
  ...flatten(en),
  'plugins.error.http_failed': 'The request failed: {reason}',
};

export const COMPARISON: Comparison = {
  path: '/shop',
  environments: [
    {
      id: 'staging',
      url: 'https://staging.example.test/shop',
      finalUrl: 'https://staging.example.test/shop',
      status: 200,
      findings: [
        { rule: 'nosniff.missing', severity: 'warning', message: 'plugin.security-headers.findings.nosniff.missing', params: { environment: 'staging' } },
        { rule: 'csp.unsafe', severity: 'error', message: 'plugin.security-headers.findings.csp.unsafe', params: { keywords: "'unsafe-inline'", environment: 'staging' } },
      ],
    },
    { id: 'prod', url: 'https://example.test/shop', finalUrl: 'https://example.test/shop', status: 200, findings: [] },
    { id: 'local', url: 'http://localhost:3000/shop', error: { code: 'plugins.error.http_failed', params: { reason: 'connection refused' } } },
  ],
  rows: [
    { header: 'content-security-policy', values: { staging: "script-src 'self' 'unsafe-inline'", prod: "script-src 'self'", local: null }, differs: true },
    { header: 'strict-transport-security', values: { staging: 'max-age=31536000', prod: 'max-age=31536000', local: null }, differs: false },
    { header: 'x-content-type-options', values: { staging: null, prod: 'nosniff', local: null }, differs: true },
    { header: 'set-cookie', cookie: 'sid', values: { staging: 'HttpOnly', prod: 'Secure; HttpOnly; SameSite=Lax', local: null }, differs: true },
    { header: 'set-cookie', cookie: 'pref', values: { staging: '', prod: '', local: null }, differs: false },
  ],
  differences: 3,
};
