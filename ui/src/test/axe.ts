// Runs axe-core over a mounted page of the shell (RB-090-08) with the WCAG 2.2 A and AA rules.
//
// The tests run in happy-dom, which lays nothing out: axe cannot measure what it cannot see.
// `color-contrast` is left to `assets/contrast.test.ts`, which measures every token pair in both
// themes (RB-090-22); `target-size` needs real boxes and stays with the manual pass in
// `docs/accessibility.md`. Everything else - names, roles, states, labels, ARIA, lists, ids,
// nested interactive controls - axe checks on the DOM as it is.

import axe from 'axe-core';

/** The rule sets the job names: WCAG 2.0, 2.1 and 2.2, A and AA. */
export const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

/** Rules that need a layout engine; see the head of this file. */
const NEEDS_LAYOUT = ['color-contrast', 'target-size'];

/** One violation, short enough to read in a failed assertion. */
export interface Violation {
  rule: string;
  help: string;
  targets: string[];
}

/** The WCAG violations axe finds in `root` (the document's body when left out). */
export async function axeViolations(root: Element = document.body): Promise<Violation[]> {
  const result = await axe.run(root, {
    runOnly: { type: 'tag', values: WCAG_TAGS },
    rules: Object.fromEntries(NEEDS_LAYOUT.map((rule) => [rule, { enabled: false }])),
    resultTypes: ['violations'],
  });
  return result.violations.map((violation) => ({
    rule: violation.id,
    help: violation.help,
    targets: violation.nodes.map((node) => node.target.join(' ')),
  }));
}
