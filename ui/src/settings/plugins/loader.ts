// The installed plugins as the core reports them, asked for by Settings > Plugins (RB-080-51):
// while the core still loads its plugins it is asked again, once a second, up to a minute, and
// then the page says so with Retry. One chain of requests at a time: a refresh or leaving the
// page starts a new number, and an answer or a timer of an older chain does nothing. The page
// holds one loader for its plugin tabs (RB-080-80), so the counts and the lists agree. An
// install, update or removal takes effect live (RB-092-06): the core's `pluginsChanged` asks for
// a list shown already again ([`refreshOnChange`]).

import { computed, ref, type ComputedRef, type Ref } from 'vue';
import type { IpcClient } from '../../ipc/client';
import type { CoreToShell } from '../../ipc/protocol';
import { type InstalledList, parseInstalled } from './installed';

/** While the core still loads its plugins the page asks again, this often at most. */
const RETRY_MS = 1000;
const MAX_RETRIES = 60;

export interface InstalledLoader {
  list: Ref<InstalledList | null>;
  loadError: Ref<string | null>;
  /** The core still loaded after every retry: an error with Retry instead of "loading" for good. */
  timedOut: Ref<boolean>;
  failed: ComputedRef<boolean>;
  refresh(): void;
  /** Ends the chain: no more requests until the next refresh. */
  stop(): void;
}

export function createInstalledLoader(client: IpcClient): InstalledLoader {
  const list = ref<InstalledList | null>(null);
  const loadError = ref<string | null>(null);
  const timedOut = ref(false);
  let retries = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let chain = 0;

  function stop(): void {
    chain += 1;
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  async function load(mine: number): Promise<void> {
    let next: InstalledList;
    try {
      const answer = await client.expect({ type: 'pluginPanel', request: { op: 'installed' } }, 'pluginPanel');
      next = parseInstalled(answer.message);
    } catch (error) {
      if (mine === chain) {
        loadError.value = error instanceof Error ? error.message : String(error);
      }
      return;
    }
    if (mine !== chain) {
      return;
    }
    list.value = next;
    loadError.value = null;
    if (!next.loading) {
      return;
    }
    if (retries >= MAX_RETRIES) {
      timedOut.value = true;
      return;
    }
    retries += 1;
    timer = setTimeout(() => {
      timer = null;
      void load(mine);
    }, RETRY_MS);
  }

  function refresh(): void {
    stop();
    retries = 0;
    timedOut.value = false;
    void load(chain);
  }

  return {
    list,
    loadError,
    timedOut,
    failed: computed(() => loadError.value !== null || timedOut.value),
    refresh,
    stop,
  };
}

/** Whether `message` says the installed plugins changed: an install, update or removal took
 * effect live (RB-092-06). */
export function isPluginsChanged(message: CoreToShell): boolean {
  return message.type === 'pluginPanel' && 'pluginsChanged' in message.message;
}

/** Asks `loader` for its list again whenever the core says the installed plugins changed; a list
 * never asked for stays unasked. Returns the function that stops listening. */
export function refreshOnChange(client: Pick<IpcClient, 'on'>, loader: InstalledLoader): () => void {
  return client.on((message) => {
    if (isPluginsChanged(message) && loader.list.value !== null) {
      loader.refresh();
    }
  });
}
