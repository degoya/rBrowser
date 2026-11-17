// The Vue panel's Router tab (RB-080-31), pure: the kit's router info (`getRouterInfo`, the
// `router-info-updated` broadcast: the current route and the route records of
// `router.getRoutes()`, each with its original `children`) as a tree of routes for the tree
// pane and as rows of the current route or a selected record for the state pane.

import { formatValue, type StateRow, type TreeNode } from './vue-model';
import { isObject } from '../../../web-common/src/is-object';

/** A route record as the kit filters it. */
export interface RouteRecord {
  path: string;
  name?: unknown;
  meta?: unknown;
  children?: unknown;
}

/** The routes as a tree, and every record by its node id. */
export interface RouterTree {
  nodes: TreeNode[];
  byId: Map<string, RouteRecord>;
}

function isRecord(value: unknown): value is RouteRecord {
  return isObject(value) && typeof value.path === 'string';
}

function nameTag(record: RouteRecord): { label: string }[] {
  return typeof record.name === 'string' || typeof record.name === 'symbol'
    ? [{ label: String(record.name) }]
    : [];
}

function nodesOf(records: unknown, parent: string | null, byId: Map<string, RouteRecord>): TreeNode[] {
  if (!Array.isArray(records)) return [];
  const nodes: TreeNode[] = [];
  for (const record of records) {
    if (!isRecord(record)) continue;
    const id = parent === null ? record.path : `${parent}>${record.path}`;
    byId.set(id, record);
    const node: TreeNode = { id, label: record.path, tags: nameTag(record) };
    const children = nodesOf(record.children, id, byId);
    if (children.length > 0) node.children = children;
    nodes.push(node);
  }
  return nodes;
}

/** The route records of `info` as a tree; a child's id is its parent's and its own path. */
export function routerTree(info: unknown): RouterTree {
  const byId = new Map<string, RouteRecord>();
  const nodes = isObject(info) ? nodesOf(info.routes, null, byId) : [];
  return { nodes, byId };
}

function current(info: unknown): Record<string, unknown> | null {
  if (!isObject(info) || !isObject(info.currentRoute)) return null;
  return typeof info.currentRoute.path === 'string' ? info.currentRoute : null;
}

/** The current route's path, or null before the router spoke. */
export function currentPath(info: unknown): string | null {
  const route = current(info);
  return route === null ? null : String(route.path);
}

function objectRows(section: string, value: unknown): StateRow[] {
  if (!isObject(value)) return [];
  return Object.entries(value).map(([key, entry]) => ({ section, key, value: formatValue(entry) }));
}

/** The rows of the current route: its fields, params, query and the matched records' paths.
 *  The sections are ids the panel names (`panel.sections.<id>`). */
export function currentRouteRows(info: unknown): StateRow[] {
  const route = current(info);
  if (route === null) return [];
  const section = 'current';
  const rows: StateRow[] = [];
  for (const key of ['path', 'fullPath', 'name', 'hash']) {
    if (route[key] !== undefined) rows.push({ section, key, value: formatValue(route[key]) });
  }
  rows.push(...objectRows('params', route.params), ...objectRows('query', route.query));
  if (Array.isArray(route.matched)) {
    route.matched.forEach((record: unknown, index) => {
      if (isRecord(record)) rows.push({ section: 'matched', key: String(index), value: formatValue(record.path) });
    });
  }
  return rows;
}

/** The rows of one route record: path, name and its meta. */
export function routeRows(record: RouteRecord | undefined): StateRow[] {
  if (record === undefined) return [];
  const rows: StateRow[] = [{ section: 'route', key: 'path', value: formatValue(record.path) }];
  if (record.name !== undefined && record.name !== null) {
    rows.push({ section: 'route', key: 'name', value: formatValue(String(record.name)) });
  }
  rows.push(...objectRows('meta', record.meta));
  return rows;
}
