import { describe, expect, it } from 'vitest';
import { countIn, folderName, foldersWith, isCollectionName, joinFolder, moveTargets, treeOf } from './collections';

const REQUESTS = [
  { id: 'health.http', name: 'Health', method: 'GET', url: '{{baseUrl}}/health' },
  { id: 'users/list.http', name: 'List users', method: 'GET', url: '{{baseUrl}}/users' },
  { id: 'users/admin/ban.http', name: 'Ban', method: 'POST', url: '{{baseUrl}}/ban' },
];

describe('the collections (RB-096-11)', () => {
  it('lists every folder, the empty ones too, each under its parent', () => {
    expect(foldersWith(REQUESTS, ['empty', 'users'])).toEqual(['empty', 'users', 'users/admin']);
    const tree = treeOf(REQUESTS, ['empty']);
    expect(tree.map((folder) => [folder.folder, folder.depth, folder.requests.length])).toEqual([
      ['', 0, 1],
      ['empty', 1, 0],
      ['users', 1, 1],
      ['users/admin', 2, 1],
    ]);
  });

  it('filters by title, URL or method and leaves out the folders without a match', () => {
    const found = treeOf(REQUESTS, ['empty'], 'post');
    expect(found.map((folder) => folder.folder)).toEqual(['users/admin']);
  });

  it('counts the requests a delete takes with the folder', () => {
    expect(countIn(REQUESTS, 'users')).toBe(2);
    expect(countIn(REQUESTS, 'users/admin')).toBe(1);
    expect(countIn(REQUESTS, 'use')).toBe(0);
  });

  it('names a folder by its last step and joins a new one below a parent', () => {
    expect(folderName('users/admin')).toBe('admin');
    expect(joinFolder('users', ' team ')).toBe('users/team');
    expect(joinFolder('', 'team/')).toBe('team');
  });

  it('takes a collection name as the core does', () => {
    expect(isCollectionName('users')).toBe(true);
    expect(isCollectionName('users/admin')).toBe(true);
    for (const bad of ['', '.git', '../x', 'a\\b', 'x.http', 'a/b/c/d/e', 'a//b']) expect(isCollectionName(bad), bad).toBe(false);
  });

  it('offers every other folder to move a request to, the top among them', () => {
    expect(moveTargets(['empty', 'users'], 'users/list.http')).toEqual(['', 'empty']);
    expect(moveTargets(['users'], 'health.http')).toEqual(['users']);
  });
});
