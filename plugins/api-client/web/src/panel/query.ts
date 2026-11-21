// The URL's query as the parameter table shows it (RB-096-01): split and joined without decoding,
// so a `{{variable}}` and an escape the person typed stay as typed.

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
