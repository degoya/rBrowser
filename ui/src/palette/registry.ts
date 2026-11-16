// The palette's providers (RB-060-11): the built-in ones and whatever a feature registers
// (`registerPaletteProvider`, see `README.md`). A provider that throws is skipped, so one broken
// source never empties the palette.

import { flowsProvider } from './flowsProvider';
import { commandsProvider, settingsProvider } from './providers';
import { stateProviders } from './stateProviders';
import type { PaletteEntry, PaletteProvider, ProviderContext } from './types';

export interface ProviderRegistry {
  /** Adds `provider` (replacing one of the same id); the returned function removes it again. */
  register(provider: PaletteProvider): () => void;
  /** Every provider's entries at open time; the first entry of a key wins. */
  collect(context: ProviderContext): PaletteEntry[];
  /** The entries that depend on the typed text. */
  collectForQuery(query: string, context: ProviderContext): PaletteEntry[];
}

function safely(run: () => PaletteEntry[]): PaletteEntry[] {
  try {
    return run();
  } catch {
    return [];
  }
}

function unique(entries: PaletteEntry[]): PaletteEntry[] {
  const seen = new Set<string>();
  return entries.filter((entry) => !seen.has(entry.key) && seen.add(entry.key));
}

export function createProviderRegistry(initial: readonly PaletteProvider[] = []): ProviderRegistry {
  const providers = new Map<string, PaletteProvider>(initial.map((p) => [p.id, p]));
  return {
    register(provider) {
      providers.set(provider.id, provider);
      return () => {
        if (providers.get(provider.id) === provider) {
          providers.delete(provider.id);
        }
      };
    },
    collect: (context) => unique([...providers.values()].flatMap((p) => safely(() => p.entries(context)))),
    collectForQuery: (query, context) =>
      unique(
        [...providers.values()].flatMap((p) =>
          p.forQuery === undefined ? [] : safely(() => p.forQuery?.(query, context) ?? []),
        ),
      ),
  };
}

/** The shell's palette providers: the registry's commands, the state's lists, the flows, the settings. */
export const BUILTIN_PROVIDERS: readonly PaletteProvider[] = [
  commandsProvider,
  ...stateProviders,
  flowsProvider,
  settingsProvider,
];

/** The one registry the shell's palette reads. */
export const paletteProviders: ProviderRegistry = createProviderRegistry(BUILTIN_PROVIDERS);

/** Adds a provider to the shell's palette; the returned function removes it. */
export function registerPaletteProvider(provider: PaletteProvider): () => void {
  return paletteProviders.register(provider);
}
