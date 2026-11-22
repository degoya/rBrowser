// What a ZIP made on Unix carries beyond its files (RB-097-02): Playwright's macOS WebKit archives
// hold executables (`pw_run.sh`, the app's binary, the XPC services) and the frameworks' symbolic
// links (`Versions/Current -> A`, `WebKit -> Versions/Current/WebKit`, the XPC services four
// folders up). The archive is checked against its pinned SHA-256 first; these rules still trust
// nothing in it: a link's target is relative and stays inside the folder however it is resolved -
// through the archive's other links too -, no entry lies under a link (nothing is written through
// one), and the links are made after every file. Permissions are the owner's, group's and others'
// read, write and execute bits only - never setuid, setgid or sticky.

import { ZipError } from './zip_error.ts';

const S_IFMT = 0o170000;
const S_IFLNK = 0o120000;
/** How many links one path may pass through (the kernel's ELOOP limit). */
const MAX_HOPS = 40;

/** Whether a Unix `st_mode` is a symbolic link. */
export function isLink(mode: number): boolean {
  return (mode & S_IFMT) === S_IFLNK;
}

/** The permissions a file gets from its `st_mode`; 0o644 when the archive says none. */
export function fileMode(mode: number): number {
  const bits = mode & 0o777;
  return bits === 0 ? 0o644 : bits;
}

/** Refuses the archive's links unless each stays inside the folder: `links` maps each link's
 *  entry name to its target, `names` are every entry's names. */
export function checkLinks(names: readonly string[], links: ReadonlyMap<string, string>): void {
  for (const [link, target] of links) {
    if (target === '' || target.includes('\0') || target.includes('\\') || target.startsWith('/')) {
      throw new ZipError(`the link ${link} points outside the folder (${JSON.stringify(target)})`);
    }
  }
  for (const name of names) {
    for (const link of links.keys()) {
      if (name.startsWith(`${link}/`)) throw new ZipError(`${name} lies under the link ${link}`);
    }
  }
  for (const [link, target] of links) {
    const folder = link.split('/').slice(0, -1);
    resolve([...folder, ...target.split('/')], links, 0, link);
  }
}

/** The folder-relative path `parts` lead to, every link on the way followed; refuses a path
 *  that climbs above the folder or follows too many links. */
function resolve(parts: readonly string[], links: ReadonlyMap<string, string>, hops: number, link: string): string[] {
  const at: string[] = [];
  for (const part of parts) {
    if (part === '' || part === '.') continue;
    if (part === '..') {
      if (at.length === 0) throw new ZipError(`the link ${link} points outside the folder`);
      at.pop();
      continue;
    }
    at.push(part);
    const target = links.get(at.join('/'));
    if (target === undefined) continue;
    if (hops >= MAX_HOPS) throw new ZipError(`the link ${link} follows too many links`);
    at.pop();
    const resolved = resolve([...at, ...target.split('/')], links, hops + 1, link);
    at.splice(0, at.length, ...resolved);
  }
  return at;
}
