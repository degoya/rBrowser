// What the tree's actions do (RB-096-11): each asks its question first where it needs one
// (`TreeDialog.vue`), then runs the core's op - `createFolder`, `renameFolder`,
// `duplicateFolder`, `deleteFolder`, `duplicate`, `move`, a title saved with `load` and `save` -,
// reads the collections again and says what happened. The editor follows: a request deleted or
// moved while open is closed or keeps its new id.
import { ref } from 'vue';
import type { Tone } from '../../../../web-common/panel-ui/src/status';
import { type Dialog, countIn, moveTargets } from './collections';
import type { TreeAction } from './menus';
import type { ApiRequest, State } from './model';

export interface CollectionDeps {
  op: <T>(body: Record<string, unknown>) => Promise<T>;
  t: (key: string, params?: Record<string, unknown>) => string;
  state: () => State | null;
  refresh: () => Promise<void>;
  say: (tone: Tone, text: string) => void;
  fail: (error: unknown) => void;
  /** The id in the editor, or null. */
  opened: () => string | null;
  open: (id: string) => Promise<void>;
  /** A new request in the editor, to be saved in `folder`. */
  freshIn: (folder: string) => void;
  /** The export tab, set to `folder`. */
  exportFolder: (folder: string) => void;
  /** The request in the editor is gone (`null`) or has a new id. */
  renamed: (id: string | null) => void;
}

export function createCollections(deps: CollectionDeps) {
  const dialog = ref<Dialog | null>(null);

  function folders(): string[] {
    return deps.state()?.folders ?? [];
  }

  async function run(work: () => Promise<string>): Promise<void> {
    try {
      const said = await work();
      await deps.refresh();
      deps.say('ok', said);
    } catch (error) {
      deps.fail(error);
    }
  }

  function act(action: TreeAction): void {
    const requests = deps.state()?.requests ?? [];
    switch (action.kind) {
      case 'open':
        void deps.open(action.id);
        return;
      case 'duplicate':
        void run(async () => {
          const name = requests.find((request) => request.id === action.id)?.name ?? '';
          const copy = await deps.op<{ id: string }>({ op: 'duplicate', id: action.id, name: name ? deps.t('panel.copyName', { name }) : null });
          return deps.t('panel.duplicated', { id: copy.id });
        });
        return;
      case 'rename':
        dialog.value = { kind: 'rename', id: action.id, name: requests.find((request) => request.id === action.id)?.name ?? '' };
        return;
      case 'move':
        dialog.value = { kind: 'move', id: action.id, targets: moveTargets(folders(), action.id) };
        return;
      case 'delete':
        dialog.value = { kind: 'delete', id: action.id };
        return;
      case 'newRequest':
        deps.freshIn(action.folder);
        return;
      case 'newFolder':
        dialog.value = { kind: 'newFolder', parent: action.folder };
        return;
      case 'renameFolder':
        dialog.value = { kind: 'renameFolder', folder: action.folder };
        return;
      case 'duplicateFolder':
        void run(async () => {
          const copy = await deps.op<{ folder: string; count: number }>({ op: 'duplicateFolder', folder: action.folder });
          return deps.t('panel.folderDuplicated', { folder: copy.folder, count: copy.count });
        });
        return;
      case 'exportFolder':
        deps.exportFolder(action.folder);
        return;
      case 'deleteFolder':
        dialog.value = { kind: 'deleteFolder', folder: action.folder, count: countIn(requests, action.folder) };
        return;
    }
  }

  /** The person's answer to the open question. */
  function answer(value: string): void {
    const asked = dialog.value;
    dialog.value = null;
    if (!asked) return;
    const opened = deps.opened();
    switch (asked.kind) {
      case 'newFolder':
        void run(async () => {
          const made = await deps.op<{ folder: string }>({ op: 'createFolder', folder: value });
          return deps.t('panel.folderMade', { folder: made.folder });
        });
        return;
      case 'renameFolder':
        void run(async () => {
          const made = await deps.op<{ folder: string }>({ op: 'renameFolder', folder: asked.folder, to: value });
          if (opened?.startsWith(`${asked.folder}/`)) deps.renamed(made.folder + opened.slice(asked.folder.length));
          return deps.t('panel.folderRenamed', { folder: made.folder });
        });
        return;
      case 'rename':
        void run(async () => {
          const loaded = await deps.op<{ request: ApiRequest }>({ op: 'load', id: asked.id });
          await deps.op({ op: 'save', id: asked.id, request: { ...loaded.request, name: value } });
          if (opened === asked.id) await deps.open(asked.id);
          return deps.t('panel.saved', { id: asked.id });
        });
        return;
      case 'move':
        void run(async () => {
          const moved = await deps.op<{ id: string }>({ op: 'move', id: asked.id, folder: value });
          if (opened === asked.id) deps.renamed(moved.id);
          return deps.t('panel.moved', { id: moved.id });
        });
        return;
      case 'delete':
        void run(async () => {
          await deps.op({ op: 'delete', id: asked.id });
          if (opened === asked.id) deps.renamed(null);
          return deps.t('panel.deleted', { id: asked.id });
        });
        return;
      case 'deleteFolder':
        void run(async () => {
          const gone = await deps.op<{ count: number }>({ op: 'deleteFolder', folder: asked.folder });
          if (opened?.startsWith(`${asked.folder}/`)) deps.renamed(null);
          return deps.t('panel.folderDeleted', { folder: asked.folder, count: gone.count });
        });
        return;
    }
  }

  return { dialog, act, answer, cancel: () => (dialog.value = null) };
}
