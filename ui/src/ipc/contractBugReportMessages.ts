// The bug report's messages of the contract test's typed literals (`contract.test.ts`,
// RB-093-04), in a file of its own so the core list stays under the file-length limit.

import type { CoreToShell, ShellToCore } from './protocol';

export const bugReportShellMessages = [
  { type: 'bugReport', request: { op: 'decide', previewId: 4, save: true } },
] satisfies ShellToCore[];

export const bugReportCoreMessages = [
  {
    type: 'bugReport',
    answer: {
      kind: 'preview',
      previewId: 4,
      folder: 'C:/work/shop/.rbrowser/out/reports/2026-11-20_14-03-07',
      markdown: '# Bug report\n',
      images: 3,
      annotate: false,
    },
  },
] satisfies CoreToShell[];
