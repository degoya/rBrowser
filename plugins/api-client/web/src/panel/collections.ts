// The collections in the panel's tree (RB-096-11): every folder below .rbrowser/api - an empty
// one too, as the state's `folders` lists it - with its requests, nested by depth; the counts a
// delete names, the names a new collection may take (the core checks them again), the folders a
// request may move to.
import { type Listed, folderOf } from './model';

/** The most folder levels the core keeps (rb_api_model::paths::MAX_DEPTH). */
export const MAX_DEPTH = 4;

/** The question a tree action asks before it runs (`TreeDialog.vue`). */
export type Dialog =
  | { kind: 'newFolder'; parent: string }
  | { kind: 'renameFolder'; folder: string }
  | { kind: 'rename'; id: string; name: string }
  | { kind: 'move'; id: string; targets: string[] }
  | { kind: 'delete'; id: string }
  | { kind: 'deleteFolder'; folder: string; count: number };

export interface TreeFolder {
  /** The folder's path, `''` for the top. */
  folder: string;
  depth: number;
  requests: Listed[];
}

/** Every folder: the given ones, those of the requests and every one above them, sorted. */
export function foldersWith(requests: readonly Listed[], folders: readonly string[]): string[] {
  const all = new Set<string>();
  const add = (folder: string): void => {
    const steps = folder.split('/').filter((step) => step !== '');
    for (let depth = 1; depth <= steps.length; depth += 1) all.add(steps.slice(0, depth).join('/'));
  };
  folders.forEach(add);
  requests.forEach((request) => add(folderOf(request.id)));
  return [...all].sort((a, b) => a.localeCompare(b));
}

function matches(request: Listed, needle: string): boolean {
  return [request.name, request.url, request.id, request.method].some((text) => text.toLowerCase().includes(needle));
}

/** The tree: the top's requests, then each folder with its own, in path order; with a query
 * only the requests that match and their folders. */
export function treeOf(requests: readonly Listed[], folders: readonly string[], query = ''): TreeFolder[] {
  const needle = query.trim().toLowerCase();
  const shown = needle ? requests.filter((request) => matches(request, needle)) : requests;
  const out: TreeFolder[] = [{ folder: '', depth: 0, requests: shown.filter((request) => folderOf(request.id) === '') }];
  for (const folder of foldersWith(requests, folders)) {
    const own = shown.filter((request) => folderOf(request.id) === folder);
    if (needle && own.length === 0) continue;
    out.push({ folder, depth: folder.split('/').length, requests: own });
  }
  return out.filter((folder) => folder.folder !== '' || folder.requests.length > 0);
}

/** How many requests lie in `folder` and below it. */
export function countIn(requests: readonly Listed[], folder: string): number {
  return requests.filter((request) => request.id.startsWith(`${folder}/`)).length;
}

/** The folder's own name: its last step. */
export function folderName(folder: string): string {
  return folder.slice(folder.lastIndexOf('/') + 1);
}

/** `name` below `parent` (`''` the top), trimmed of spaces and slashes. */
export function joinFolder(parent: string, name: string): string {
  const own = name.trim().replace(/^\/+|\/+$/g, '');
  return parent ? `${parent}/${own}` : own;
}

const SEGMENT = /^[A-Za-z0-9 _.-]{1,80}$/;

/** Whether `folder` may be a collection: as the core's `collection_ok`. */
export function isCollectionName(folder: string): boolean {
  const steps = folder.split('/');
  return (
    folder !== '' &&
    steps.length <= MAX_DEPTH &&
    steps.every((step) => SEGMENT.test(step) && !step.startsWith('.') && !step.endsWith(' ') && !step.endsWith('.http'))
  );
}

/** The folders request `id` may move to: every one but its own, the top first. */
export function moveTargets(folders: readonly string[], id: string): string[] {
  const own = folderOf(id);
  return ['', ...folders].filter((folder) => folder !== own);
}
