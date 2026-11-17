// A path the native dialog returned, as a project key stores it (RB-080-20): a folder inside
// the open project becomes relative to it with forward slashes (`.rbrowser/out/shots`), as the
// project's keys require; anything else stays as chosen.

function normalise(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
}

/** `path` relative to `root` when it lies inside; `path` as it is otherwise. */
export function projectRelative(path: string, root: string | null | undefined): string {
  if (root === null || root === undefined || root === '') {
    return path;
  }
  const base = normalise(root);
  const candidate = normalise(path);
  const inside =
    candidate.length > base.length + 1 &&
    candidate.slice(0, base.length).toLowerCase() === base.toLowerCase() &&
    candidate[base.length] === '/';
  return inside ? candidate.slice(base.length + 1) : path;
}
