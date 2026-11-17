import { describe, expect, it } from 'vitest';
import { currentPath, currentRouteRows, routeRows, routerTree } from './vue-router-model';

const info = {
  currentRoute: {
    fullPath: '/users/7?tab=posts',
    path: '/users/7',
    name: 'user',
    hash: '',
    params: { id: '7' },
    query: { tab: 'posts' },
    matched: [{ path: '/users/:id', name: 'user' }],
  },
  routes: [
    { path: '/', name: 'home', meta: {} },
    {
      path: '/users/:id',
      name: 'user',
      meta: { requiresAuth: true },
      children: [{ path: 'posts', name: 'user-posts' }],
    },
  ],
};

describe('routerTree', () => {
  it('turns the routes into tree nodes with the name as a tag, ids unique down the tree', () => {
    const { nodes, byId } = routerTree(info);
    expect(nodes).toEqual([
      { id: '/', label: '/', tags: [{ label: 'home' }] },
      {
        id: '/users/:id',
        label: '/users/:id',
        tags: [{ label: 'user' }],
        children: [{ id: '/users/:id>posts', label: 'posts', tags: [{ label: 'user-posts' }] }],
      },
    ]);
    expect([...byId.keys()]).toEqual(['/', '/users/:id', '/users/:id>posts']);
  });

  it('is empty for anything that is not router info', () => {
    expect(routerTree(null).nodes).toEqual([]);
    expect(routerTree({ routes: 'x' }).nodes).toEqual([]);
    expect(routerTree({ routes: [{ name: 'no path' }, 'x'] }).nodes).toEqual([]);
  });
});

describe('currentRouteRows', () => {
  it('lists the current route, its params, query and matched records', () => {
    expect(currentRouteRows(info)).toEqual([
      { section: 'current', key: 'path', value: '"/users/7"' },
      { section: 'current', key: 'fullPath', value: '"/users/7?tab=posts"' },
      { section: 'current', key: 'name', value: '"user"' },
      { section: 'current', key: 'hash', value: '""' },
      { section: 'params', key: 'id', value: '"7"' },
      { section: 'query', key: 'tab', value: '"posts"' },
      { section: 'matched', key: '0', value: '"/users/:id"' },
    ]);
  });

  it('is empty without a current route', () => {
    expect(currentRouteRows({ currentRoute: null, routes: [] })).toEqual([]);
    expect(currentRouteRows({ currentRoute: {}, routes: [] })).toEqual([]);
    expect(currentPath({ currentRoute: {}, routes: [] })).toBeNull();
    expect(currentPath(info)).toBe('/users/7');
  });
});

describe('routeRows', () => {
  it('lists a route record with its meta', () => {
    const { byId } = routerTree(info);
    expect(routeRows(byId.get('/users/:id'))).toEqual([
      { section: 'route', key: 'path', value: '"/users/:id"' },
      { section: 'route', key: 'name', value: '"user"' },
      { section: 'meta', key: 'requiresAuth', value: 'true' },
    ]);
    expect(routeRows(byId.get('/users/:id>posts'))).toEqual([
      { section: 'route', key: 'path', value: '"posts"' },
      { section: 'route', key: 'name', value: '"user-posts"' },
    ]);
    expect(routeRows(undefined)).toEqual([]);
  });
});
