// The sitemap start page's state (RB-099-09, `SitemapStartPage.vue`): the discovery asked once
// the page opens (`sitemapStart` `discover`, with the core's own budget for reading a site's
// sitemaps), cancelled by the person (a later answer is dropped), the start of the chosen pages
// (`sitemap.capture` with `discovery` and `urls`; the core closes the window once the run has
// started) and Close. The last choice of views and environment is remembered in the page's
// storage - no setting (owner decision, 2026-11-23).

import { ref, type Ref } from 'vue';
import type { IpcClient } from '../../ipc/client';
import type { JsonValue, SitemapDiscovery } from '../../ipc/protocol';
import type { ViewsChoice } from './tree';

/** `sitemap.capture`, which takes the chosen pages. */
export const CAPTURE_COMMAND = 'sitemap.capture';
/** How long a discovery may take: the core's `DISCOVER_BUDGET` and a margin. */
export const DISCOVER_TIMEOUT_MS = 310_000;
/** Where the last choice is kept. */
export const CHOICE_KEY = 'rbrowser.sitemapStart.choice';

export type StartState =
  | { phase: 'finding' }
  | { phase: 'cancelled' }
  | { phase: 'failed'; message: string }
  | { phase: 'found'; discovery: SitemapDiscovery }
  | { phase: 'starting'; discovery: SitemapDiscovery }
  | { phase: 'refused'; discovery: SitemapDiscovery; message: string };

export interface Start {
  state: Ref<StartState>;
  /** Finds the pages (again); an answer of an earlier call is dropped. */
  discover(): Promise<void>;
  /** Stops waiting for the discovery in progress. */
  cancel(): void;
  /** Starts the run of `args` (`sitemap.capture`); the core closes the window. */
  start(args: { [key: string]: JsonValue }): Promise<void>;
  /** Closes the start page's window. */
  close(): Promise<void>;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function createStart(client: IpcClient): Start {
  const state = ref<StartState>({ phase: 'finding' });
  let asked = 0;

  async function discover(): Promise<void> {
    asked += 1;
    const mine = asked;
    state.value = { phase: 'finding' };
    try {
      const answer = await client.request({ type: 'sitemapStart', op: 'discover' }, { timeoutMs: DISCOVER_TIMEOUT_MS });
      if (mine !== asked) return;
      if (answer.type !== 'sitemapDiscovery') {
        state.value = { phase: 'failed', message: `unexpected ${answer.type}` };
        return;
      }
      const { type: _type, ...discovery } = answer;
      state.value = { phase: 'found', discovery };
    } catch (error: unknown) {
      if (mine === asked) state.value = { phase: 'failed', message: messageOf(error) };
    }
  }

  function cancel(): void {
    if (state.value.phase !== 'finding') return;
    asked += 1;
    state.value = { phase: 'cancelled' };
  }

  async function start(args: { [key: string]: JsonValue }): Promise<void> {
    const current = state.value;
    if (current.phase !== 'found' && current.phase !== 'refused') return;
    const { discovery } = current;
    state.value = { phase: 'starting', discovery };
    try {
      await client.request({ type: 'runCommand', id: CAPTURE_COMMAND, args });
    } catch (error: unknown) {
      state.value = { phase: 'refused', discovery, message: messageOf(error) };
    }
  }

  async function close(): Promise<void> {
    try {
      await client.request({ type: 'sitemapStart', op: 'close' });
    } catch (error: unknown) {
      console.warn('the sitemap start page could not be closed', error);
    }
  }

  return { state, discover, cancel, start, close };
}

/** The views and environment chosen last. */
export interface Choice {
  views: ViewsChoice;
  env: string | null;
}

const DEFAULT_CHOICE: Choice = { views: 'all', env: null };

/** The last choice in `storage`; the default when there is none, or none can be read. */
export function loadChoice(storage: Pick<Storage, 'getItem'> | null): Choice {
  try {
    const text = storage?.getItem(CHOICE_KEY);
    if (text === null || text === undefined) return DEFAULT_CHOICE;
    const value = JSON.parse(text) as Partial<Choice>;
    return {
      views: value.views === 'visible' ? 'visible' : 'all',
      env: typeof value.env === 'string' ? value.env : null,
    };
  } catch {
    return DEFAULT_CHOICE;
  }
}

/** Keeps `choice` in `storage`; a storage that refuses keeps nothing. */
export function saveChoice(storage: Pick<Storage, 'setItem'> | null, choice: Choice): void {
  try {
    storage?.setItem(CHOICE_KEY, JSON.stringify(choice));
  } catch {
    // A private window or blocked site data: the choice is not remembered.
  }
}

/** The page's storage, or none where it cannot be reached. */
export function pageStorage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}
