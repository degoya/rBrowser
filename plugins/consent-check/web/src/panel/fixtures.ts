// The consent check panel's test fixtures (RB-094-05): the English texts by full key and an
// answer of `check` from the probe session of the acceptance page - a script of a second site
// and a cookie kept a year before consent, a pixel and a cookie more after "Reject all".
import en from '../../../locales/en.json';
import type { Answer } from './model';

function flatten(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
export const STRINGS = flatten(en);

const NOTICE = 'Technical pre-check, no legal advice.';

export const ANSWER: Answer = {
  mode: 'session',
  url: 'https://shop.example.test/',
  site: 'example.test',
  view: null,
  notice: 'plugin.consent-check.notice',
  noticeText: NOTICE,
  caveats: [],
  rejectStep: true,
  banner: { cmp: 'onetrust', name: 'OneTrust', visible: true, rejected: true },
  session: { fresh: true, discarded: true },
  before: {
    hosts: [{ host: 'cdn.tracker.test', kind: 'script', count: 1, allowed: false, example: 'https://cdn.tracker.test/t.js' }],
    firstParty: 2,
    cookies: [{ name: '_track', domain: '.shop.example.test', days: 365, essential: false, thirdParty: false }],
    storage: [],
  },
  afterReject: {
    hosts: [{ host: 'pixel.ads.test', kind: 'pixel', count: 1, allowed: false, example: 'https://pixel.ads.test/p.gif' }],
    firstParty: 1,
    cookies: [
      { name: '_track', domain: '.shop.example.test', days: 365, essential: false, thirdParty: false },
      { name: '_ads', domain: '.ads.test', days: null, essential: false, thirdParty: true },
      { name: 'OptanonConsent', domain: '.shop.example.test', days: 365, essential: true, thirdParty: false },
    ],
    storage: ['consent-state'],
  },
  diff: {
    hostsAdded: ['pixel.ads.test pixel'],
    hostsRemoved: ['cdn.tracker.test script'],
    cookiesAdded: ['OptanonConsent .shop.example.test', '_ads .ads.test'],
    cookiesRemoved: [],
    storageAdded: ['consent-state'],
    storageRemoved: [],
  },
  findings: [
    {
      rule: 'request.before_consent',
      severity: 'warning',
      message: 'plugin.consent-check.findings.request.before_consent',
      params: [['host', 'cdn.tracker.test'], ['kind', 'script'], ['count', '1']],
    },
    {
      rule: 'cookie.before_consent',
      severity: 'warning',
      message: 'plugin.consent-check.findings.cookie.before_consent',
      params: [['name', '_track'], ['domain', '.shop.example.test'], ['days', '365']],
    },
    {
      rule: 'request.after_reject',
      severity: 'error',
      message: 'plugin.consent-check.findings.request.after_reject',
      params: [['host', 'pixel.ads.test'], ['kind', 'pixel'], ['count', '1']],
    },
    {
      rule: 'cookie.after_reject',
      severity: 'error',
      message: 'plugin.consent-check.findings.cookie.after_reject_session',
      params: [['name', '_ads'], ['domain', '.ads.test']],
    },
  ],
  summary: { error: 2, warning: 2 },
};

/** The answer of the view mode: no step after "Reject all", its caveats. */
export const VIEW_ANSWER: Answer = {
  ...ANSWER,
  mode: 'view',
  view: 1,
  caveats: ['viewSession', 'firstPartyCookies', 'noRejectStep'],
  banner: { cmp: null, name: null, visible: false, rejected: false },
  session: { fresh: false, discarded: null },
  afterReject: null,
  diff: null,
  findings: ANSWER.findings.slice(0, 2),
  summary: { error: 0, warning: 2 },
};
