# Changes

## Unreleased

- Removed the setting `json-xml-viewer.inFirefox` (Charge 10, core-fixes): page scripts reach
  only Chromium views, so it never had an effect. Firefox keeps its own JSON viewer; a value
  left in the user settings is ignored.

## 0.1.0

First release (RB-070-05): a JSON document (`application/json`, `text/json`, `*+json`) or an XML
document (`application/xml`, `text/xml`, `*+xml` - sitemaps, RSS, Atom) opened in a view is shown
as a collapsible tree in a closed shadow root, keyboard-operable as an ARIA tree, in light or dark
colours (`json-xml-viewer.theme`). Virtual rendering keeps large documents fluid; the first
`json-xml-viewer.expandDepth` levels open within a row budget; above `json-xml-viewer.maxBytes`
(20 MiB) a document stays plain text with a notice. The filter takes plain text, JSONPath
(`$.items[*].id`, no filter expressions) or XPath (`//url/loc`, unprefixed names match any
namespace); "Copy value" and "Copy path" copy the active row. Invalid JSON shows its raw text
with the line and column of the error marked. The command `plugin.json_xml_viewer.toggle_raw`
(`Ctrl+Shift+Space V J`) switches between tree and raw text; Firefox views keep their own viewer
unless `json-xml-viewer.inFirefox` is on.
