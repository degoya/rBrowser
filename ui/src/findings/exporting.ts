// The findings export of the panel (RB-092-05): the command `checks.export` writes the last run as
// a file - the format the setting `checks.export.defaultFormat` names, or the one chosen from the
// Export button's arrow - and names the file in the status bar.

import type { FindingsRequest } from './types';

/** The command the Export button runs (`Ctrl+Shift+Space K B`, MCP `checks_export`). */
export const EXPORT_COMMAND = 'checks.export';

/** The formats the arrow offers, in its order. */
export const EXPORT_FORMATS = ['sarif', 'junit', 'html', 'json'] as const;

export type ExportFormat = (typeof EXPORT_FORMATS)[number];

/** The request that exports in `format`; `null` leaves the format to the setting. */
export function exportRequest(format: ExportFormat | null): FindingsRequest {
  return format === null
    ? { type: 'runCommand', id: EXPORT_COMMAND }
    : { type: 'runCommand', id: EXPORT_COMMAND, args: { format } };
}
