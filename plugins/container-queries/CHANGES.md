# Changes

## Unreleased

- Changed (RB-094-10): a container's `path` (`containers_analyze`, `containers_resize`, the
  panel) is the element path every check plugin shares (`web-common/src/element-path.ts`,
  `html > body > main > article`) instead of its id or the child positions; `containers_resize`
  still takes any selector, `#hero` included.

## 0.1.0

The first release (RB-094-01): the container overlay in every view - each element with a size
`container-type` framed with its `container-name`, its inline and block size and the
`@container` conditions that match now, a handle that drags its width inline (a double click
resets it) - the commands `plugin.container_queries.overlay` (`Ctrl+Shift+Space O U`) and
`plugin.container_queries.analyze` (`Ctrl+Shift+Space B C`), the MCP tools
`containers_overlay`, `containers_analyze` and `containers_resize`, the panel with every
condition and its bounds and each container per view with the next bound below and above, the
breakpoints that switch it and its reset, and the settings `container-queries.overlay.labels`
and `container-queries.overlay.color` under DevTools & Debugging > Container queries.
