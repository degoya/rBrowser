import { describe, expect, it } from 'vitest';
import { hostOf, isOpenableUrl, openLinkRequest, openLinksIn } from './open';

describe('links out of rBrowser', () => {
  it('sends only web addresses', () => {
    expect(isOpenableUrl('https://dequeuniversity.com/rules/axe/4.10/color-contrast')).toBe(true);
    expect(isOpenableUrl('http://localhost:5173/')).toBe(true);
    for (const url of [
      'javascript:alert(1)',
      'file:///C:/Windows/System32/calc.exe',
      'rbrowser://settings/ui.theme',
      'data:text/html,x',
      'https://user:pw@bank.example/',
      'https://site.test/ x',
      '',
      `https://site.test/${'a'.repeat(2048)}`,
      undefined,
      42,
    ]) {
      expect(isOpenableUrl(url), String(url)).toBe(false);
    }
  });

  it('reads the setting, the browser when it says nothing known', () => {
    expect(openLinksIn({})).toBe('system');
    expect(openLinksIn({ 'ui.openLinksIn': 'ask' })).toBe('ask');
    expect(openLinksIn({ 'ui.openLinksIn': 'activeView' })).toBe('activeView');
    expect(openLinksIn({ 'ui.openLinksIn': 'newWindow' })).toBe('system');
  });

  it('asks the core with the command, the place only when chosen', () => {
    expect(openLinkRequest('https://help.test/')).toEqual({
      type: 'runCommand',
      id: 'app.open_link',
      args: { url: 'https://help.test/' },
    });
    expect(openLinkRequest('https://help.test/', 'activeView')).toEqual({
      type: 'runCommand',
      id: 'app.open_link',
      args: { url: 'https://help.test/', where: 'activeView' },
    });
    expect(hostOf('https://help.test:8443/a')).toBe('help.test:8443');
  });
});
