// The Sessions state of the network panel (RB-080-37, on Nuxt UI since RB-080-84): the captured
// requests saved as a HAR session (`session_save`), the saved sessions listed (`sessions_list`)
// and one opened for display (`session_open`) - the Requests tab then shows the session's rows
// until "back to live".
import { ref } from 'vue';
import type { Panel } from '../../../../web-common/panel-ui/src/panel';
import type { SessionAnswer } from './types';

export type SessionsStore = ReturnType<typeof createSessions>;

export function createSessions(panel: Panel, show: (session: SessionAnswer | null) => void) {
  const list = ref<{ name: string; path: string }[]>([]);
  const note = ref('');

  async function refresh(): Promise<void> {
    try {
      const value = (await panel.call({ tool: 'sessions_list' })) as { sessions: { name: string; path: string }[] };
      if (value && Array.isArray(value.sessions)) list.value = value.sessions;
    } catch (error) {
      note.value = panel.error(error);
    }
  }

  function save(): void {
    panel.call({ tool: 'session_save' }).then(
      (value) => {
        note.value = panel.t('sessions.saved', { path: (value as { path: string }).path });
        void refresh();
      },
      (error: unknown) => {
        note.value = panel.error(error);
      },
    );
  }

  function open(path: string): void {
    panel.call({ tool: 'session_open' }, { path }).then(
      (value) => {
        const opened = value as SessionAnswer;
        if (!opened || !Array.isArray(opened.rows)) return;
        note.value = panel.t('sessions.opened', { path: opened.path, entries: opened.total });
        show(opened);
      },
      (error: unknown) => {
        note.value = panel.error(error);
      },
    );
  }

  return { list, note, refresh, save, open, close: () => show(null) };
}
