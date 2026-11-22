import { describe, expect, it } from 'vitest';
import { joinUrl, rowsFor, splitUrl } from './query';

describe('the query table', () => {
  it('splits a URL without decoding and joins it back', () => {
    const url = '{{baseUrl}}/users?page=2&q=a%20b&flag#top';
    const split = splitUrl(url);
    expect(split).toEqual({
      base: '{{baseUrl}}/users',
      params: [
        { name: 'page', value: '2' },
        { name: 'q', value: 'a%20b' },
        { name: 'flag', value: '' },
      ],
      hash: '#top',
    });
    expect(joinUrl(split.base, split.params, split.hash)).toBe(url);
  });

  it('leaves rows without a name out and the ? away without parameters', () => {
    expect(joinUrl('https://a.example/x', [{ name: '', value: '1' }])).toBe('https://a.example/x');
    expect(joinUrl('https://a.example/x', [{ name: 'k', value: '{{key}}' }])).toBe('https://a.example/x?k={{key}}');
    expect(splitUrl('https://a.example/').params).toEqual([]);
  });

  // RB-096-13: "Add" put a nameless row into the URL, which leaves it out - the row was gone at
  // once. The table keeps its rows while they make the URL, and reads the URL again when it
  // changed elsewhere.
  it('keeps the table\'s rows while they make the URL, a new nameless one included', () => {
    const rows = [
      { name: 'page', value: '2' },
      { name: '', value: '' },
    ];
    expect(rowsFor('https://a.example/x?page=2', rows)).toBe(rows);
    expect(rowsFor('https://a.example/x?page=3', rows)).toEqual([{ name: 'page', value: '3' }]);
    expect(rowsFor('https://a.example/x', [{ name: '', value: '' }])).toEqual([{ name: '', value: '' }]);
  });
});
