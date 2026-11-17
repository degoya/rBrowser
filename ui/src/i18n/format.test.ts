import { describe, expect, it } from 'vitest';
import formatCases from '@locales/tests/format-cases.json';
import { formatMessage, pluralCategory, pseudoLocalize, validateMessage } from './format';
import { createShellI18n } from './index';

type FormatCase = {
  name: string;
  locale: string;
  message: string;
  args: Record<string, string | undefined>;
  expected: string;
};

const cases: FormatCase[] = formatCases.cases;
const invalid: string[] = formatCases.invalid;

describe('the shared format cases (locales/tests/format-cases.json)', () => {
  it.each(cases.map((c) => [c.name, c] as const))('formats: %s', (_name, c) => {
    expect(validateMessage(c.message)).toBeNull();
    expect(formatMessage(c.message, c.args, c.locale)).toBe(c.expected);
  });

  it.each(invalid)('refuses %j and shows it as written', (message) => {
    expect(validateMessage(message)).toEqual(expect.any(String));
    expect(formatMessage(message, { n: '1', url: 'a.test' })).toBe(message);
  });
});

describe('the message format', () => {
  it('applies the English plural rule to the argument as text', () => {
    expect(pluralCategory('1')).toBe('one');
    expect(pluralCategory('01')).toBe('one');
    expect(pluralCategory('-1')).toBe('one');
    expect(pluralCategory('1.0')).toBe('other');
    expect(pluralCategory('0')).toBe('other');
    expect(pluralCategory('')).toBe('other');
  });

  it('matches an exact selector by number, falling back to other', () => {
    const message = '{n, plural, =1 {exactly one} few {few} other {# others}}';
    expect(formatMessage(message, { n: '1.0' })).toBe('exactly one');
    expect(formatMessage(message, { n: 'x' })).toBe('x others');
    expect(formatMessage(message, { n: 2 })).toBe('2 others');
  });

  it('allows spaces inside the braces of a placeholder', () => {
    expect(formatMessage('Open { url }', { url: 'a.test' })).toBe('Open a.test');
  });

  it('pseudo-localizes, empty staying empty', () => {
    expect(pseudoLocalize('Reload')).toBe('[Rélóád~]');
    expect(pseudoLocalize('')).toBe('');
  });
});

describe('the shell message compiler', () => {
  const i18n = () => {
    const { global } = createShellI18n();
    global.mergeLocaleMessage('en', {
      test: {
        views: '{count, plural, =0 {No views} one {# view} other {# views}}',
        literal: "Can't #1 a | b @:shell.toolbar.back",
        broken: '{count, plural, one {# view}}',
      },
    });
    return global;
  };

  // Keys through constants: the i18n lint holds every literal `t('...')` key to locales/en.json,
  // and these test messages exist only in this test.
  const VIEWS = 'test.views';
  const LITERAL = 'test.literal';
  const BROKEN = 'test.broken';

  it('formats a plural through vue-i18n', () => {
    const { t } = i18n();
    expect(t(VIEWS, { count: 0 })).toBe('No views');
    expect(t(VIEWS, { count: 1 })).toBe('1 view');
    expect(t(VIEWS, { count: 3 })).toBe('3 views');
    expect(t(VIEWS)).toBe('{count, plural, =0 {No views} one {# view} other {# views}}');
  });

  it("reads none of vue-i18n's own syntax", () => {
    expect(i18n().t(LITERAL)).toBe("Can't #1 a | b @:shell.toolbar.back");
  });

  it('shows a message that does not parse as written', () => {
    expect(i18n().t(BROKEN, { count: 1 })).toBe('{count, plural, one {# view}}');
  });
});
