import { describe, expect, it } from 'vitest';
import { sampleState } from '../ipc/mockState';
import {
  addMessage,
  bookmarksOf,
  filterBookmarks,
  followsEnvironment,
  groupBookmarks,
  openMessage,
  targetOf,
} from './bookmarks';

describe('bookmarks', () => {
  it('lists the core\'s bookmarks, none without a state', () => {
    expect(bookmarksOf(sampleState()).map((b) => b.id)).toEqual(['home', 'pdp', 'checkout', 'storybook']);
    expect(bookmarksOf(null)).toEqual([]);
  });

  it('groups by group, the ungrouped first, groups in order of appearance', () => {
    const groups = groupBookmarks(bookmarksOf(sampleState()));
    expect(groups.map((g) => [g.name, g.items.map((b) => b.id)])).toEqual([
      [null, ['home']],
      ['Shop', ['pdp', 'checkout']],
      ['Tools', ['storybook']],
    ]);
    expect(groupBookmarks([])).toEqual([]);
  });

  it('finds every typed word in label, group or URL', () => {
    const list = bookmarksOf(sampleState());
    expect(filterBookmarks(list, '').map((b) => b.id)).toEqual(['home', 'pdp', 'checkout', 'storybook']);
    expect(filterBookmarks(list, 'shop').map((b) => b.id)).toEqual(['pdp', 'checkout']);
    expect(filterBookmarks(list, 'SHOP check').map((b) => b.id)).toEqual(['checkout']);
    expect(filterBookmarks(list, '6006').map((b) => b.id)).toEqual(['storybook']);
    expect(filterBookmarks(list, 'nothing')).toEqual([]);
  });

  it('reads a typed target as a path or an http(s) URL', () => {
    expect(targetOf(' /checkout ')).toEqual({ path: '/checkout' });
    expect(targetOf('https://x.example/a')).toEqual({ url: 'https://x.example/a' });
    expect(targetOf('HTTP://x.example')).toEqual({ url: 'HTTP://x.example' });
    expect(targetOf('checkout')).toBeNull();
    expect(targetOf('javascript:alert(1)')).toBeNull();
    expect(targetOf('/a b')).toBeNull();
    expect(targetOf('https://')).toBeNull();
  });

  it('refuses a path a browser reads as another host (RB-080-48)', () => {
    expect(targetOf('//evil.example')).toBeNull();
    expect(targetOf('//evil.example/checkout')).toBeNull();
    expect(targetOf('/\\evil.example')).toBeNull();
    expect(targetOf('/a\\b')).toBeNull();
    expect(targetOf('/redirect?to=https://a.example//b')).toEqual({ path: '/redirect?to=https://a.example//b' });
  });

  it('follows the environment unless the setting says no', () => {
    expect(followsEnvironment({})).toBe(true);
    expect(followsEnvironment({ 'bookmarks.followEnvironment': true })).toBe(true);
    expect(followsEnvironment({ 'bookmarks.followEnvironment': false })).toBe(false);
  });

  it('runs the commands with their arguments: what was typed and the switch, the core decides the form', () => {
    expect(openMessage('pdp')).toEqual({ type: 'runCommand', id: 'bookmarks.open', args: { id: 'pdp' } });
    expect(
      addMessage({ label: ' Checkout ', target: { path: '/checkout' }, follow: true, group: 'Shop', scope: 'local' }),
    ).toEqual({
      type: 'runCommand',
      id: 'bookmarks.add',
      args: { label: 'Checkout', path: '/checkout', follow: true, group: 'Shop', scope: 'local' },
    });
    expect(
      addMessage({ label: '', target: { url: 'https://x.example' }, follow: false, group: ' ', scope: 'project' }),
    ).toEqual({
      type: 'runCommand',
      id: 'bookmarks.add',
      args: { url: 'https://x.example', follow: false, scope: 'project' },
    });
  });
});
