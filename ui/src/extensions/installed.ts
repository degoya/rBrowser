// The settings' list of the extensions installed from the Chrome Web Store (RB-080-30): the core
// answers `listWebStoreExtensions` with `ack` and sends the rows as the event
// `webStoreExtensions`, which it sends again after every change of the store (an install, an
// update, a removal, by the person or by MCP), so the list is never stale. Remove runs the
// person's command `extensions.remove` with the id as `ref`; the core deletes the folder and
// ends in a notice. What this start loaded stays loaded until the next start, which the row says.

import type { IpcClient } from '../ipc/client';
import type { WebStoreExtension } from '../ipc/protocol';

/** The command a row's Remove runs. */
export const REMOVE_COMMAND = 'extensions.remove';

/** Asks for the list; it arrives as `webStoreExtensions` (`onInstalled`). */
export async function requestInstalled(client: IpcClient): Promise<void> {
  await client.request({ type: 'listWebStoreExtensions' });
}

/** Calls `listed` with every `webStoreExtensions`; returns the function that stops. */
export function onInstalled(client: IpcClient, listed: (items: WebStoreExtension[]) => void): () => void {
  return client.on((message) => {
    if (message.type === 'webStoreExtensions') listed(message.items);
  });
}

/** Runs `extensions.remove` for `id`; the core's notice says what happened, the list follows. */
export async function removeInstalled(client: IpcClient, id: string): Promise<void> {
  await client.request({ type: 'runCommand', id: REMOVE_COMMAND, args: { ref: id } });
}
