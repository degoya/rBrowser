// The cookies an answer sets (RB-096-01): every `Set-Cookie` header as its name, value and
// attributes, for the panel's Cookies tab.

export interface Cookie {
  name: string;
  value: string;
  attributes: { name: string; value: string }[];
}

/** The cookies of `headers` (name and value pairs, any case). */
export function cookiesOf(headers: readonly (readonly [string, string])[]): Cookie[] {
  return headers
    .filter(([name]) => name.toLowerCase() === 'set-cookie')
    .map(([, line]) => {
      const [pair = '', ...rest] = line.split(';');
      const eq = pair.indexOf('=');
      const attributes = rest
        .map((part) => part.trim())
        .filter((part) => part !== '')
        .map((part) => {
          const at = part.indexOf('=');
          return at < 0 ? { name: part, value: '' } : { name: part.slice(0, at).trim(), value: part.slice(at + 1).trim() };
        });
      return eq < 0
        ? { name: pair.trim(), value: '', attributes }
        : { name: pair.slice(0, eq).trim(), value: pair.slice(eq + 1).trim(), attributes };
    });
}
