# Changes

## Unreleased

- The element is found without dropping node ids other users of the view's shared CDP session
  hold (the document is requested only when nobody has; a selector starts from the document the
  accessibility tree's root names), and no CDP domain is switched on any more: a view whose CSS
  domain will not name the rendered fonts shows "n/a" (review finding L6 of Charge 6).

## 0.1.0

First release (RB-060-04): holding Alt (or Ctrl or Shift, `peek.modifier`) over an element shows
its tag, id, classes, size in CSS and device pixels, text and background colour with the WCAG
contrast and level (large-text rule, background resolved through the ancestors, "uncertain" over
images and gradients), declared and - in Chromium - rendered font, box model, role and accessible
name; the same element is outlined in every other view (`peek.syncHighlight`). A click while the
modifier is held locks the tooltip, with a copy button. The command `plugin.peek.toggle`
(`Ctrl+Shift+Space D P`) switches the inspector; the MCP tool `peek_inspect` answers the same
data as JSON.
