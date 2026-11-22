// The URL's query as the parameter table shows it (RB-096-01): split and joined without decoding,
// so a `{{variable}}` and an escape the person typed stay as typed. The table keeps its own rows
// while they make the URL - a row just added has no name yet and is not in the URL (RB-096-13).

export interface Param {
  name: string;
  value: string;
}

export interface SplitUrl {
  base: string;
  params: Param[];
  hash: string;
}

/** `url` cut into the part before `?`, its parameters and the fragment (with its `#`). */
export function splitUrl(url: string): SplitUrl {
  const hashAt = url.indexOf('#');
  const hash = hashAt >= 0 ? url.slice(hashAt) : '';
  const rest = hashAt >= 0 ? url.slice(0, hashAt) : url;
  const queryAt = rest.indexOf('?');
  if (queryAt < 0) return { base: rest, params: [], hash };
  const params = rest
    .slice(queryAt + 1)
    .split('&')
    .filter((pair) => pair !== '')
    .map((pair) => {
      const eq = pair.indexOf('=');
      return eq < 0 ? { name: pair, value: '' } : { name: pair.slice(0, eq), value: pair.slice(eq + 1) };
    });
  return { base: rest.slice(0, queryAt), params, hash };
}

/** The URL of `base`, `params` (a row without a name is left out) and `hash`. */
export function joinUrl(base: string, params: readonly Param[], hash = ''): string {
  const pairs = params
    .filter((param) => param.name !== '')
    .map((param) => (param.value === '' ? param.name : `${param.name}=${param.value}`));
  return base + (pairs.length > 0 ? `?${pairs.join('&')}` : '') + hash;
}

/** The table's rows for `url`: `rows` while they make it (a row without a name is in no URL),
 * else the URL's own parameters - it changed elsewhere (typed, loaded). */
export function rowsFor(url: string, rows: Param[]): Param[] {
  const split = splitUrl(url);
  return joinUrl(split.base, rows, split.hash) === url ? rows : split.params;
}
