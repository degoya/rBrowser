// The bug report preview's state (RB-093-04, `BugReportPreview.vue`): the report waiting for the
// person's word, asked once when the page opens (`getPreview`), and the word sent back
// (`decide`) - once; a refused word leaves the report open with the failure named, so the person
// can try again. Loading, nothing waiting and a core that could not be asked are three states.

import { ref, type Ref } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { ReportPreview } from '../ipc/protocol';

export type PreviewState =
  | { phase: 'loading' }
  | { phase: 'none' }
  | { phase: 'failed' }
  | { phase: 'open'; preview: ReportPreview; failed?: boolean }
  | { phase: 'sending'; preview: ReportPreview }
  | { phase: 'decided'; saved: boolean };

export interface Preview {
  state: Ref<PreviewState>;
  /** Asks the core for the waiting report. */
  load(): Promise<void>;
  /** Saves or drops the open report. */
  decide(save: boolean): Promise<void>;
}

export function createPreview(client: IpcClient): Preview {
  const state = ref<PreviewState>({ phase: 'loading' });

  async function load(): Promise<void> {
    try {
      const answer = await client.request({ type: 'bugReport', request: { op: 'getPreview' } });
      if (answer.type === 'bugReport' && answer.answer.kind === 'preview') {
        const { kind: _kind, ...preview } = answer.answer;
        state.value = { phase: 'open', preview };
      } else {
        state.value = { phase: 'none' };
      }
    } catch (error: unknown) {
      console.warn('the bug report preview could not be read', error);
      state.value = { phase: 'failed' };
    }
  }

  async function decide(save: boolean): Promise<void> {
    const current = state.value;
    if (current.phase !== 'open') return;
    const { preview } = current;
    state.value = { phase: 'sending', preview };
    try {
      await client.request({ type: 'bugReport', request: { op: 'decide', previewId: preview.previewId, save } });
      state.value = { phase: 'decided', saved: save };
    } catch (error: unknown) {
      console.warn('the bug report decision was refused', error);
      state.value = { phase: 'open', preview, failed: true };
    }
  }

  return { state, load, decide };
}
