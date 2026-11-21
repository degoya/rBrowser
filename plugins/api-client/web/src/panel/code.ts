// A request as code and a folder as a file (RB-096-03): what the core's ops `code` and `export`
// answer, the panel's choices and the clipboard. The core writes the code; secrets arrive as
// the variables the code reads, never as values.
import type { Listed } from './model';

export const LANGUAGES = ['curl', 'httpie', 'fetch', 'playwright'] as const;
export type Language = (typeof LANGUAGES)[number];
export const FORMATS = ['postman', 'http', 'curl'] as const;
export type Format = (typeof FORMATS)[number];
/** The folder choice for every request: a select item's value cannot be empty. */
export const ALL = '/';

export interface CodeAnswer {
  language: Language;
  text: string;
  /** The environment variables the code reads. */
  env: string[];
  /** What the code leaves out: `oauth2`, `digest`, `insecure`, `lineBreaks`, `duplicateNames`. */
  notes: string[];
  shell: string;
  environment: string | null;
}

export interface ExportAnswer {
  path: string;
  format: Format;
  count: number;
  skipped: string[];
  env: string[];
  notes: string[];
}

/** Every folder of `requests`, the ones above included, sorted. */
export function foldersOf(requests: readonly Listed[]): string[] {
  const folders = new Set<string>();
  for (const request of requests) {
    const steps = request.id.split('/').slice(0, -1);
    for (let depth = 1; depth <= steps.length; depth += 1) folders.add(steps.slice(0, depth).join('/'));
  }
  return [...folders].sort((a, b) => a.localeCompare(b));
}

/** The highlighter's language of a code language. */
export function highlightOf(language: Language): 'js' | null {
  return language === 'fetch' || language === 'playwright' ? 'js' : null;
}

/** Writes `text` to the clipboard; the sandboxed frame may refuse the async clipboard, then the
 * old way on a hidden field. False when neither works. */
export async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const field = document.createElement('textarea');
    field.value = text;
    field.setAttribute('readonly', '');
    field.style.position = 'fixed';
    field.style.opacity = '0';
    document.body.append(field);
    field.select();
    let copied = false;
    try {
      copied = document.execCommand('copy');
    } catch {
      copied = false;
    }
    field.remove();
    return copied;
  }
}
