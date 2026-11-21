// The bug report's messages (RB-093-04), mirror of rb-ipc `bug_report.rs`; kept beside
// `protocol.ts`, which re-exports them. The preview page (`#/bug-report` of the pages window)
// asks `getPreview` and answers with `decide`; the status bar's copy button asks `getMarkdown`
// for a saved report's masked text and copies it itself - the core has no clipboard.

/** A report gathered and masked, not saved yet: what the preview shows. */
export interface ReportPreview {
  /** The preview's number, for the decision. */
  previewId: number;
  /** The report's folder. */
  folder: string;
  /** `report.md` as it will be saved: every text masked. */
  markdown: string;
  /** The screenshots taken (password fields blacked out), the overview counted. */
  images: number;
  /** `report.annotate`: the screenshot editor opens once the report is saved. */
  annotate: boolean;
}

export type BugReportRequest =
  /** Answered with `bugReport` (`preview`), `ack` when no report waits. */
  | { op: 'getPreview' }
  /** Save or drop the waiting report; `ack`. The pages window's only. */
  | { op: 'decide'; previewId: number; save: boolean }
  /** A saved report's masked `report.md`, by its folder's name; answered with `bugReport` (`markdown`). */
  | { op: 'getMarkdown'; report: string };

export type BugReportAnswer =
  | ({ kind: 'preview' } & ReportPreview)
  | { kind: 'markdown'; report: string; text: string };

export interface BugReportShellMessage {
  type: 'bugReport';
  request: BugReportRequest;
}

export interface BugReportCoreMessage {
  type: 'bugReport';
  answer: BugReportAnswer;
}
