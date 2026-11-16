// What the Web Store field accepts (RB-080-23), the shell's copy of
// `crates/rbrowser/src/extensions/webstore.rs::parse_ref`: an extension id (32 letters a to p)
// or a store page URL with the id as the first or second path segment after `detail/`.

/** The command the field runs. */
export const INSTALL_COMMAND = 'extensions.install_web_store';

const STORE_PAGES = ['https://chromewebstore.google.com/detail/', 'https://chrome.google.com/webstore/detail/'];

/** Whether `id` is an extension id. */
export function isExtensionId(id: string): boolean {
  return /^[a-p]{32}$/.test(id);
}

/** The id `written` names, or `null`. */
export function parseStoreRef(written: string): string | null {
  const trimmed = written.trim();
  if (isExtensionId(trimmed)) {
    return trimmed;
  }
  const prefix = STORE_PAGES.find((page) => trimmed.startsWith(page));
  if (prefix === undefined) {
    return null;
  }
  const path = trimmed.slice(prefix.length).split(/[?#]/)[0] ?? '';
  const segments = path.split('/').filter((segment) => segment !== '').slice(0, 2);
  return segments.find(isExtensionId) ?? null;
}
