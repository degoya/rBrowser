// The context menus of the collections' tree (RB-096-11), as `UContextMenu` items: on a request
// open, duplicate, rename, move to a collection and delete; on a collection a new request, a new
// collection inside it, rename, duplicate, export and delete; on the top a new request and a new
// collection. Every item hands its action to the panel, which asks before a delete.
import type { ContextMenuItem } from '@nuxt/ui';

export type RequestAction = 'open' | 'duplicate' | 'rename' | 'move' | 'delete';
export type FolderAction = 'newRequest' | 'newFolder' | 'renameFolder' | 'duplicateFolder' | 'exportFolder' | 'deleteFolder';
export type TreeAction = { kind: RequestAction; id: string } | { kind: FolderAction; folder: string };

type T = (key: string) => string;
type Act = (action: TreeAction) => void;

function item(t: T, label: string, icon: string, onSelect: () => void, danger = false): ContextMenuItem {
  return { label: t(`panel.menu.${label}`), icon, onSelect, ...(danger ? { color: 'error' as const } : {}) };
}

/** The menu of request `id`. */
export function requestMenu(id: string, t: T, act: Act): ContextMenuItem[][] {
  const on = (kind: RequestAction) => () => act({ kind, id });
  return [
    [
      item(t, 'open', 'i-lucide-file-text', on('open')),
      item(t, 'duplicate', 'i-lucide-copy', on('duplicate')),
      item(t, 'rename', 'i-lucide-pencil', on('rename')),
      item(t, 'move', 'i-lucide-folder-input', on('move')),
    ],
    [item(t, 'delete', 'i-lucide-trash', on('delete'), true)],
  ];
}

/** The menu of collection `folder` (`''` the top). */
export function folderMenu(folder: string, t: T, act: Act): ContextMenuItem[][] {
  const on = (kind: FolderAction) => () => act({ kind, folder });
  const make = [item(t, 'newRequest', 'i-lucide-file-plus', on('newRequest')), item(t, 'newFolder', 'i-lucide-folder-plus', on('newFolder'))];
  if (folder === '') return [make];
  return [
    make,
    [
      item(t, 'rename', 'i-lucide-pencil', on('renameFolder')),
      item(t, 'duplicate', 'i-lucide-copy', on('duplicateFolder')),
      item(t, 'export', 'i-lucide-download', on('exportFolder')),
    ],
    [item(t, 'delete', 'i-lucide-trash', on('deleteFolder'), true)],
  ];
}
