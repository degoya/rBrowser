import { describe, expect, it } from 'vitest';
import { EXPORT_COMMAND, EXPORT_FORMATS, exportRequest } from './exporting';

describe('exportRequest', () => {
  it('leaves the format to the setting without a choice', () => {
    expect(exportRequest(null)).toEqual({ type: 'runCommand', id: EXPORT_COMMAND });
  });

  it('names the chosen format', () => {
    expect(exportRequest('junit')).toEqual({ type: 'runCommand', id: 'checks.export', args: { format: 'junit' } });
  });

  it('offers the four formats of the core', () => {
    expect([...EXPORT_FORMATS]).toEqual(['sarif', 'junit', 'html', 'json']);
  });
});
