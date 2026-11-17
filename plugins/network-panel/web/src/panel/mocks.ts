// The mocks state of the network panel (RB-070-02, on Nuxt UI since RB-080-84): the mock files and
// their rules with a switch and the hits each, the person's block list, the mock editor (a rule
// from a request, saved with `mock_add`) and the HAR import. Without workspace trust the files are
// only named, as blocked.
import { reactive, ref } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import { escapeGlob, mockArgs } from './model';
import type { MocksAnswer } from './types';

export type MocksStore = ReturnType<typeof createMocks>;

export function createMocks(panel: Panel, changed: () => void) {
  const answer = ref<MocksAnswer | null>(null);
  const note = ref('');
  const editor = reactive({ open: false, pattern: '', method: 'GET', action: 'mock', status: '500', delay: '0', body: '', error: '' });
  const harPath = ref('');

  async function refresh(): Promise<void> {
    try {
      const value = (await panel.call({ tool: 'mocks' })) as MocksAnswer;
      if (value && Array.isArray(value.files) && Array.isArray(value.block)) answer.value = value;
      note.value = answer.value && !answer.value.trusted ? panel.t('mocks.untrusted') : '';
    } catch (error) {
      note.value = panel.error(error);
    }
  }

  /** Opens the editor, filled from `request` (a rule matching exactly it) or empty. */
  function openEditor(request?: { url: string; method: string; status?: number | null }): void {
    Object.assign(editor, {
      open: true,
      pattern: request ? escapeGlob(request.url) : '',
      method: request?.method ?? 'GET',
      action: 'mock',
      status: String(request?.status && request.status >= 400 ? request.status : 500),
      delay: '0',
      body: '',
      error: '',
    });
  }

  async function save(): Promise<void> {
    const outcome = mockArgs(editor);
    if (!outcome.ok) {
      editor.error = panel.t(outcome.error);
      return;
    }
    try {
      await panel.call({ tool: 'mock_add' }, outcome.args);
      editor.open = false;
      await refresh();
      changed();
    } catch (error) {
      editor.error = panel.error(error);
    }
  }

  function importHar(): void {
    const path = harPath.value.trim();
    if (!path) return;
    panel.call({ tool: 'har_import' }, { path }).then(
      (value) => {
        const done = value as { rules: number; skipped: number };
        void refresh().then(() => {
          note.value = panel.t('mocks.imported', { rules: done.rules, skipped: done.skipped });
        });
        changed();
      },
      (error: unknown) => {
        note.value = panel.error(error);
      },
    );
  }

  return {
    answer,
    note,
    editor,
    harPath,
    refresh,
    openEditor,
    save,
    importHar,
    enable: (id: string, enabled: boolean) => void panel.call({ tool: 'mock_enable' }, { id, enabled }).then(refresh, refresh),
    remove: (id: string) => void panel.call({ tool: 'mock_remove' }, { id }).then(refresh, refresh),
  };
}
