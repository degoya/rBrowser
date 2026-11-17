// The folders a flow worker is granted (RB-090-69). Node's permission model compares a path
// exactly as the call hands it to the system: on Windows a removal (`rm` of Playwright's
// `playwright-artifacts-*` folder) asks in the long form `\\?\UNC\server\share\...` (a drive path
// as `\\?\C:\...`), which a grant of `\\server\share\...` alone does not cover. Each folder is
// therefore granted in both forms, with backslashes only.

import { win32 } from 'node:path';

/** `path` without a `\\?\UNC\` (back to `\\`) or `\\?\` prefix. */
function withoutLongPrefix(path: string): string {
  if (/^\\\\\?\\UNC\\/i.test(path)) {
    return `\\\\${path.slice('\\\\?\\UNC\\'.length)}`;
  }
  return path.startsWith('\\\\?\\') ? path.slice('\\\\?\\'.length) : path;
}

/** The forms in which `path` is granted on `platform`: on Windows the plain and the long form. */
export function grantForms(path: string, platform: NodeJS.Platform): string[] {
  if (platform !== 'win32') {
    return [path];
  }
  const plain = win32.normalize(withoutLongPrefix(path.replaceAll('/', '\\')));
  const long = win32.toNamespacedPath(plain);
  return long === plain ? [plain] : [plain, long];
}
