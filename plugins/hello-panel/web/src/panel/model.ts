// What the hello-panel panel reads from the host's events (RB-060-12, RB-080-87): the component's
// `{ greeted }` after `greet`, a page script's `{ loaded, view }` and the component's
// `{ page, title, view }` for a page that said hello. Anything else is not the panel's.

export interface Page {
  /** The view the page is in; null when the event names none. */
  view: number | null;
  url: string;
}

export type HelloEvent = { greeted: string } | { page: Page };

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;

/** The event `message` (from view `view`, when the host names one) as the panel shows it. */
export function readEvent(message: unknown, view: number | undefined): HelloEvent | null {
  if (!isObject(message)) return null;
  if (typeof message.greeted === 'string') return { greeted: message.greeted };
  const url = typeof message.page === 'string' ? message.page : message.loaded;
  if (typeof url !== 'string') return null;
  const own = typeof message.view === 'number' ? message.view : null;
  return { page: { view: view ?? own, url } };
}
