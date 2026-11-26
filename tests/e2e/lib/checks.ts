// The comparisons of the criteria that hold the app to a snapshot the repository already keeps:
// K5 to tests/fixtures/analyzer/<fixture>/expected-static.json, K8 to
// plugins/seo/tests/snapshots/hreflang-*.txt (the plugin's own snapshot test writes them).

/** `[id, width, height]` of each breakpoint of a project config, as the analyser fixtures list them. */
export function viewsOf(config: unknown): [string, number, number][] {
  const breakpoints = (config as { breakpoints?: unknown } | null)?.breakpoints;
  if (!Array.isArray(breakpoints)) return [];
  return breakpoints.map((entry) => {
    const { id, width, height } = entry as { id?: unknown; width?: unknown; height?: unknown };
    return [String(id), Number(width), Number(height)];
  });
}

/**
 * The keys of a findings snapshot (plugins/seo/tests/hreflang.rs `lines`): per line its
 * severity, rule, message and path - the parameters name the fixtures' own origin and stay out.
 */
export function parseSnapshot(text: string): string[] {
  return text
    .split('\n')
    .map((line) => line.trim().split(/\s+/).slice(0, 4))
    .filter((parts) => parts.length === 4)
    .map((parts) => parts.join(' '));
}

export interface Finding {
  severity?: string;
  rule?: string;
  message?: string;
  url?: string;
}

/** The key of a `seo_check` finding, its URL below `origin` as the snapshot's path. */
export function findingKey(finding: Finding, origin: string): string {
  const message = (finding.message ?? '').replace(/^plugin\.seo\.findings\./, '');
  const path = finding.url?.startsWith(origin) ? finding.url.slice(origin.length) || '/' : '-';
  return `${finding.severity ?? '?'} ${finding.rule ?? '?'} ${message} ${path}`;
}

/** The keys `expected` has more often than `actual` (missing), and the other way (unexpected). */
export function diffKeys(expected: readonly string[], actual: readonly string[]): { missing: string[]; unexpected: string[] } {
  const rest = [...actual];
  const missing: string[] = [];
  for (const key of expected) {
    const index = rest.indexOf(key);
    if (index === -1) missing.push(key);
    else rest.splice(index, 1);
  }
  return { missing, unexpected: rest };
}

/** `[workspace.package] version` of Cargo.toml, the one version (scripts/set-version.sh; M12). */
export function workspaceVersion(cargoToml: string): string {
  const section = cargoToml.split(/^\[workspace\.package\]$/m)[1] ?? '';
  return /^version\s*=\s*"([^"]+)"/m.exec(section)?.[1] ?? '';
}
