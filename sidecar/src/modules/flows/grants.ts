// The folders a flow worker is granted (RB-090-69). Node's permission model compares a path
// exactly as the call hands it to the system: on Windows a removal (`rm` of Playwright's
// `playwright-artifacts-*` folder) asks in the long form `\\?\UNC\server\share\...` (a drive path
// as `\\?\C:\...`), which a grant of `\\server\share\...` alone does not cover. Each folder is
// therefore granted in both forms, with backslashes only. On macOS (RB-097-03) the system's own
// folders are symbolic links (`/tmp`, `/var` into `/private`), and a module loader that resolves
// a file asks under its real path, so a folder is granted also under that one when it differs.

import { win32 } from 'node:path';

/** `path` without a `\\?\UNC\` (back to `\\`) or `\\?\` prefix. */
function withoutLongPrefix(path: string): string {
  if (/^\\\\\?\\UNC\\/i.test(path)) {
    return `\\\\${path.slice('\\\\?\\UNC\\'.length)}`;
  }
  return path.startsWith('\\\\?\\') ? path.slice('\\\\?\\'.length) : path;
}

/**
 * The forms in which `path` is granted on `platform`: on Windows the plain and the long form, on
 * macOS the path and its real path (`realPath`, when given and found), elsewhere the path.
 */
export function grantForms(path: string, platform: NodeJS.Platform, realPath?: (path: string) => string): string[] {
  if (platform === 'darwin' && realPath !== undefined) {
    let real = path;
    try {
      real = realPath(path);
    } catch {
      // Not there (yet): the plain grant alone.
    }
    return real === path ? [path] : [path, real];
  }
  if (platform !== 'win32') {
    return [path];
  }
  const plain = win32.normalize(withoutLongPrefix(path.replaceAll('/', '\\')));
  const long = win32.toNamespacedPath(plain);
  return long === plain ? [plain] : [plain, long];
}
