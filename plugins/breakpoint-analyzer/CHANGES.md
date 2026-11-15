# Changes

## 0.2.0

The static source (RB-030-07): the breakpoints the project declares, read from the files the host
hands over (capability `project-read`, globs `**/tailwind.config.{js,cjs,mjs,ts}` and
`**/*.{css,scss,less}`) - Tailwind v3's `theme.screens` and `theme.extend.screens` (read as
literals, never run), Tailwind v4's `@theme { --breakpoint-*: ... }` over the defaults of the
entry that imports `tailwindcss` (`initial` included), Bootstrap's `$grid-breakpoints` map or its
defaults, and `$breakpoint-*`, `$bp-*`, `$screen-*` (SCSS) and `@breakpoint-*`, `@bp-*`,
`@screen-*` (Less) variables as a heuristic source. They merge with the media query boundaries
within the tolerance and give them their names; a declared width no page uses yet becomes a view
of its own (reason `declared`). `detect` answers `source` and `heuristic` per boundary, the project
files among `sources`, and `hints` for what is not statically readable (a call, a variable, a
range or a raw query) - a hint, never an error.

## 0.1.0

The first version (RB-010-13): `@media` boundaries from the page's stylesheets (`min-width`,
`max-width`, range syntax, `em`/`rem`, nested and combined conditions; `@container` as a hint),
merged within a tolerance, labelled after Tailwind or Bootstrap when recognised, and proposed as
breakpoints in the modes `all`, `simplified` and `edges`; the MCP tools `breakpoints_detect` and
`breakpoints_apply`.

`apply` counts a width as taken when the host's `existingWidths` holds it: an existing breakpoint
that takes its width from a device preset is no longer proposed again (RB-010-12).
