# Changes

## 0.1.0

The first version (RB-060-06): axe-core 4.14.0 (MPL-2.0, bundled) runs in every view's isolated
world for one run and is removed again; its violations - and, with `a11y-axe.reportIncomplete`,
its "needs review" results - become findings with the element's selector and path, merged
across the views by the check runner and shared with the seo plugin for missing alt texts, links
without a name and a missing `html lang`. Settings under Checks > Accessibility (tags, single
rules, included and excluded selectors), the MCP tool `a11y_check` and the command
`Ctrl+Shift+Space K A`.
