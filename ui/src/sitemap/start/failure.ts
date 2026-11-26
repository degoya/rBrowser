// What the sitemap start page says when the pages could not be found or the capture did not start
// (RB-099-09): the core's failure carries its code (`sitemap.error.no_pages: ...`, behind
// `command failed: ` for a refused start); a known code becomes a sentence that says what to do,
// any other the general one. The details, without the code, stay beside it.

const SENTENCES: Record<string, string> = {
  'sitemap.error.no_project': 'sitemap.start.error.no_project',
  'sitemap.error.no_source': 'sitemap.start.error.no_source',
  'sitemap.error.no_pages': 'sitemap.start.error.no_pages',
  'sitemap.error.bad_file': 'sitemap.start.error.bad_file',
  'sitemap.error.discover': 'sitemap.start.error.discover',
  'sitemap.error.busy': 'sitemap.start.error.busy',
  'sitemap.error.unknown_discovery': 'sitemap.start.error.unknown_discovery',
  'sitemap.error.foreign_url': 'sitemap.start.error.foreign_url',
  'sitemap.error.too_many_urls': 'sitemap.start.error.too_many_urls',
  'sitemap.error.no_breakpoints': 'sitemap.start.error.no_breakpoints',
  'sitemap.error.unknown_env': 'sitemap.start.error.unknown_env',
};

const CODE = /sitemap\.error\.[a-z_]+/;

/** The i18n key of the sentence for the core's failure `message`; `fallback` for an unknown
 * code. */
export function failureKey(message: string, fallback = 'sitemap.start.error.failed'): string {
  const code = CODE.exec(message)?.[0] ?? '';
  return SENTENCES[code] ?? fallback;
}

/** The details of `message` without its code: what the person may read beside the sentence. */
export function failureDetail(message: string): string {
  const found = CODE.exec(message);
  if (found === null) return message;
  const rest = message.slice(found.index + found[0].length);
  return rest.startsWith(': ') ? rest.slice(2) : rest.trim();
}
