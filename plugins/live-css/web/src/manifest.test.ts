// @vitest-environment node
// The manifest's MCP gate (security review w28): CSS can read a page's data out through
// selectors and `url()`, so `set` - CSS its caller sends, adopted in every view - is flagged
// `evaluates_input` and reaches MCP only while `mcp.allowEvaluate` is on (the host's rule; the
// panel calls it freely). `clear` sends nothing to the pages and stays open.
import { describe, expect, it } from 'vitest';
import { pluginFile } from '../../../web-common/src/load';

/** Each `[[tools]]` entry of manifest.toml: its name and whether it is flagged. */
function tools(): Map<string, boolean> {
  const entries = pluginFile('manifest.toml').split(/^\[\[/m).filter((part) => part.startsWith('tools]]'));
  return new Map(
    entries.map((entry) => [
      /^name = "([^"]+)"$/m.exec(entry)?.[1] ?? '',
      /^evaluates_input = true$/m.test(entry),
    ]),
  );
}

describe('the Live CSS manifest', () => {
  it('flags set, which adopts the caller CSS in every view, as evaluates_input', () => {
    expect(tools().get('set')).toBe(true);
  });

  it('leaves clear open', () => {
    expect(tools().get('clear')).toBe(false);
  });
});
