// "Copy Markdown" of a saved bug report's status message (RB-093-04): the core has no
// clipboard, so the status bar - the document the person clicked, which alone may write the
// clipboard - asks for the report's masked `report.md` by its folder's name (`bugReport`,
// `getMarkdown`) and copies it as plain text. A failure (the report is no longer known, the
// clipboard refused) becomes a status message of its own; it is never silent.

import type { IpcClient } from '../ipc/client';

/** The status message of a failed copy. */
export const COPY_FAILED_KEY = 'report.error.copy_failed';

/** Copies report `report`'s Markdown to `clipboard`; `true` when it is there. */
export async function copyReportMarkdown(
  client: IpcClient,
  report: string,
  clipboard: Pick<Clipboard, 'writeText'>,
): Promise<boolean> {
  try {
    const answer = await client.expect({ type: 'bugReport', request: { op: 'getMarkdown', report } }, 'bugReport');
    if (answer.answer.kind !== 'markdown' || answer.answer.report !== report) {
      throw new Error('the core answered another report');
    }
    await clipboard.writeText(answer.answer.text);
    return true;
  } catch (error: unknown) {
    console.warn('the bug report could not be copied', error);
    client.request({ type: 'reportStatus', level: 'error', messageKey: COPY_FAILED_KEY }).catch(() => undefined);
    return false;
  }
}
