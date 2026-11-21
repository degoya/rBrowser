// A JSONPath filter for the answer's tree (RB-096-01): the common subset - `$`, `.name`,
// `['name']`, `[0]`, `[-1]`, `[*]`, `.*` and `..name` (every depth). Anything else is no path.

type Step = { kind: 'child'; name: string } | { kind: 'index'; index: number } | { kind: 'all' } | { kind: 'deep'; name: string };

/** The steps of `path`, or null when it is no path of the subset. */
export function steps(path: string): Step[] | null {
  const text = path.trim();
  if (!text.startsWith('$')) return null;
  const out: Step[] = [];
  let at = 1;
  const name = /^[A-Za-z_$][\w$-]*/;
  while (at < text.length) {
    const rest = text.slice(at);
    if (rest.startsWith('..')) {
      const found = name.exec(rest.slice(2));
      if (!found) return null;
      out.push({ kind: 'deep', name: found[0] });
      at += 2 + found[0].length;
    } else if (rest.startsWith('.*')) {
      out.push({ kind: 'all' });
      at += 2;
    } else if (rest.startsWith('.')) {
      const found = name.exec(rest.slice(1));
      if (!found) return null;
      out.push({ kind: 'child', name: found[0] });
      at += 1 + found[0].length;
    } else if (rest.startsWith('[')) {
      const close = rest.indexOf(']');
      if (close < 0) return null;
      const inner = rest.slice(1, close).trim();
      if (inner === '*') out.push({ kind: 'all' });
      else if (/^-?\d+$/.test(inner)) out.push({ kind: 'index', index: Number(inner) });
      else if (/^'[^']*'$|^"[^"]*"$/.test(inner)) out.push({ kind: 'child', name: inner.slice(1, -1) });
      else return null;
      at += close + 1;
    } else {
      return null;
    }
  }
  return out;
}

function children(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  if (value !== null && typeof value === 'object') return Object.values(value as Record<string, unknown>);
  return [];
}

function deep(value: unknown, name: string, out: unknown[]): void {
  if (value !== null && typeof value === 'object' && !Array.isArray(value) && name in (value as Record<string, unknown>)) {
    out.push((value as Record<string, unknown>)[name]);
  }
  for (const child of children(value)) deep(child, name, out);
}

/** What `path` selects in `value`; null when `path` is no path. */
export function select(value: unknown, path: string): unknown[] | null {
  const parsed = steps(path);
  if (!parsed) return null;
  let current: unknown[] = [value];
  for (const step of parsed) {
    const next: unknown[] = [];
    for (const item of current) {
      if (step.kind === 'all') next.push(...children(item));
      else if (step.kind === 'deep') deep(item, step.name, next);
      else if (step.kind === 'index' && Array.isArray(item)) {
        const index = step.index < 0 ? item.length + step.index : step.index;
        if (index >= 0 && index < item.length) next.push(item[index]);
      } else if (step.kind === 'child' && item !== null && typeof item === 'object' && !Array.isArray(item)) {
        const record = item as Record<string, unknown>;
        if (Object.hasOwn(record, step.name)) next.push(record[step.name]);
      }
    }
    current = next;
  }
  return current;
}
