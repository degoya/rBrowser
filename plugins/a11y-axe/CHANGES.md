# Changes

## Unreleased

- Changed (RB-091-03): eleven more rules share their ids with the outline plugin (and the seo
  plugin for the headings), so the same problem is one finding: `page-has-heading-one`,
  `heading-order`, `empty-heading`, `landmark-one-main`, `landmark-no-duplicate-main`,
  `landmark-no-duplicate-banner`, `landmark-no-duplicate-contentinfo`,
  `landmark-banner-is-top-level`, `landmark-contentinfo-is-top-level`, `landmark-unique` and
  `region`.

## 0.1.0

The first version (RB-060-06): axe-core 4.14.0 (MPL-2.0, bundled) runs in every view's isolated
world for one run and is removed again; its violations - and, with `a11y-axe.reportIncomplete`,
its "needs review" results - become findings with the element's selector and path, merged
across the views by the check runner and shared with the seo plugin for missing alt texts, links
without a name and a missing `html lang`. Settings under Checks > Accessibility (tags, single
rules, included and excluded selectors), the MCP tool `a11y_check` and the command
`Ctrl+Shift+Space K A`.
