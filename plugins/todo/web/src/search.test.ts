// The todo panel's search (RB-110-18): terms without case and accents, `@tag` terms, the open
// switch, the headings and projects kept as context, the marks and the search together with the
// section and the tag filter.
import { describe, expect, it } from 'vitest';
import { panelSettings, readFile, visible, type Item } from './panel/model';
import { fold, marks, matches, parseQuery, searchItems, tagHit, type Search } from './panel/search';

const item = (line: number, extra: Partial<Item>): unknown => ({
  line,
  depth: 0,
  kind: 'task',
  state: 'open',
  text: `task ${line}`,
  raw: `task ${line}`,
  tags: [],
  project: '',
  archived: false,
  ...extra,
});
const heading = (line: number, level: number, text: string) => item(line, { kind: 'heading', state: undefined, level, text, raw: `${'#'.repeat(level)} ${text}` });
const project = (line: number, depth: number, text: string) => item(line, { depth, kind: 'project', state: undefined, text, raw: `${text}:` });

function items(list: unknown[]): Item[] {
  const file = readFile({ path: 'TODO.todo', rev: 'r', counts: {}, items: list });
  if (!file) throw new Error('file');
  return file.items;
}

/** A file of two Markdown sections, projects inside, an archive at the end. */
const ITEMS = items([
  heading(1, 1, 'Relaunch'),
  heading(2, 2, 'Design'),
  project(3, 0, 'Website'),
  item(4, { depth: 1, text: 'Crème brûlée for the café', tags: [{ name: 'high', value: null }] }),
  item(5, { depth: 1, text: 'Pick the colours', state: 'done', tags: [{ name: 'done', value: '26-11-14 09:30' }] }),
  project(6, 1, 'Icons'),
  item(7, { depth: 2, text: 'Nav icons', tags: [{ name: 'owner', value: 'Alex' }] }),
  item(8, { depth: 3, kind: 'note', state: undefined, text: 'A note on the nav' }),
  heading(9, 2, 'Copy'),
  item(10, { text: 'Write the CAFE menu', tags: [{ name: 'today', value: null }] }),
  heading(11, 1, 'Later'),
  item(12, { text: 'Hire a writer' }),
  project(13, 0, 'Archive'),
  item(14, { depth: 1, text: 'Old café', state: 'done', archived: true, project: 'Archive' }),
]);

const find = (text: string, openOnly = false): Search => ({ query: parseQuery(text), openOnly });
const lines = (list: Item[]): number[] => list.map((shown) => shown.line);

describe('the todo search', () => {
  it('folds case and accents, composed or not', () => {
    expect(fold('Crème BRÛLÉE')).toBe('creme brulee');
    expect(fold('Crème')).toBe('creme');
    expect(fold('\u00c4rger Stra\u00dfe')).toBe('arger stra\u00dfe');
    expect(fold('plain ascii')).toBe('plain ascii');
  });

  it('reads words and @tag terms, leaving out empty ones', () => {
    expect(parseQuery('  Café  @HIGH @ @est(2h) ')).toEqual({ words: ['cafe'], tags: ['high', 'est'] });
    expect(parseQuery('   ')).toEqual({ words: [], tags: [] });
  });

  it('matches a text or a tag, every term at once, a tag term by the start of its name', () => {
    const at = (line: number): Item => ITEMS.find((shown) => shown.line === line) as Item;
    expect(matches(at(4), find('CREME cafe'))).toBe(true);
    expect(matches(at(4), find('creme menu'))).toBe(false);
    // A word finds a tag's name or value; a tag term the tag's name.
    expect(matches(at(7), find('alex'))).toBe(true);
    expect(matches(at(4), find('@hi'))).toBe(true);
    expect(matches(at(4), find('@igh'))).toBe(false);
    expect(matches(at(5), find('@done'))).toBe(true);
    // Only open: a task left to do, nothing else.
    expect(matches(at(4), find('', true))).toBe(true);
    expect(matches(at(5), find('', true))).toBe(false);
    expect(matches(at(1), find('relaunch', true))).toBe(false);
    // The tags a term found, for their mark.
    expect(tagHit({ name: 'owner', value: 'Alex' }, parseQuery('alex'))).toBe(true);
    expect(tagHit({ name: 'high', value: null }, parseQuery('@HI'))).toBe(true);
    expect(tagHit({ name: 'high', value: null }, parseQuery('@today nav'))).toBe(false);
  });

  it('keeps the headings, projects and parents of every match as context, in file order', () => {
    expect(lines(searchItems(ITEMS, find('cafe')))).toEqual([1, 2, 3, 4, 9, 10, 13, 14]);
    expect(lines(searchItems(ITEMS, find('nav')))).toEqual([1, 2, 3, 6, 7, 8]);
    expect(lines(searchItems(ITEMS, find('@today')))).toEqual([1, 9, 10]);
    expect(lines(searchItems(ITEMS, find('writer')))).toEqual([11, 12]);
    // A heading that matches stands with its own headings.
    expect(lines(searchItems(ITEMS, find('copy')))).toEqual([1, 9]);
    expect(searchItems(ITEMS, find('nothing like it'))).toEqual([]);
  });

  it('shows the open todos alone with their context', () => {
    expect(lines(searchItems(ITEMS, find('', true)))).toEqual([1, 2, 3, 4, 6, 7, 9, 10, 11, 12]);
    expect(lines(searchItems(ITEMS, find('@done', true)))).toEqual([]);
  });

  it('applies together with the section, the tag filter, finished todos hidden and the archive', () => {
    const settings = panelSettings({});
    // No search: the list as before.
    expect(lines(visible(ITEMS, settings, null, 0, find('')).active)).toEqual(lines(visible(ITEMS, settings, null, 0).active));
    const all = visible(ITEMS, settings, null, 0, find('cafe'));
    expect(lines(all.active)).toEqual([1, 2, 3, 4, 9, 10]);
    expect(lines(all.archive)).toEqual([14]);
    // Inside the section "## Copy": its own match only, the section's head as the context.
    const section = visible(ITEMS, settings, null, 9, find('cafe'));
    expect(lines(section.active)).toEqual([9, 10]);
    expect(section.archive).toEqual([]);
    expect(lines(visible(ITEMS, settings, null, 9, find('creme')).active)).toEqual([]);
    // With a tag chosen: only its tasks match, still with their context.
    expect(lines(visible(ITEMS, settings, 'today', 0, find('cafe')).active)).toEqual([1, 9, 10]);
    // Finished todos hidden: a done match is no match.
    expect(lines(visible(ITEMS, { ...settings, hideDone: true }, null, 0, find('colours')).active)).toEqual([]);
    expect(visible(ITEMS, { ...settings, hideArchive: true }, null, 0, find('cafe')).archive).toEqual([]);
  });

  it('marks every place a word stands, without case and accents', () => {
    expect(marks('Crème brûlée for the café', ['creme', 'cafe'])).toEqual([
      { text: 'Crème', hit: true },
      { text: ' brûlée for the ', hit: false },
      { text: 'café', hit: true },
    ]);
    expect(marks('Crème', ['creme'])).toEqual([{ text: 'Crème', hit: true }]);
    expect(marks('banana', ['ana'])).toEqual([
      { text: 'b', hit: false },
      { text: 'anana', hit: true },
    ]);
    expect(marks('Nav icons', [])).toEqual([{ text: 'Nav icons', hit: false }]);
    expect(marks('', ['x'])).toEqual([]);
  });
});
