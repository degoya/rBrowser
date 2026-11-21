# Changes

## 0.1.0

The first version (RB-090-96): the schema.org data of the page a view shows - every JSON-LD
block (each `@graph` entry on its own), Microdata and RDFa item - as one tree of numbered nodes
with their types, `@id`, source and properties, `@id` references resolved to the node they name.
The collector (`src/collect.js`, through `evaluate` in the plugin's isolated world, read only)
sends the rendered document; the readers, the schema.org 30.1 vocabulary and the rich-result
table are the shared library `plugins/schema-org`, which the SEO check uses too. Rules:
`jsonld.invalid`, `jsonld.context`, `type.missing`, `type.unknown`, `property.unknown`,
`property.domain`, `property.range` (dates, times, numbers, booleans, enumeration members and
item types), `property.superseded`, and the rich-result needs and recommendations
(`rich.required`, `rich.required_any`, `rich.recommended`). The MCP tool
`structureddata_inspect` (tree, raw JSON-LD and findings at their nodes, within a `budget`), the
check for `checks_run`, the command `check` (Ctrl+Shift+Space K T) and the panel (Ctrl+Shift+Space
P U): the tree, the findings with a jump to their node, the raw JSON-LD blocks; Read again follows
the chosen view. Settings: `structured-data.richResults`, `structured-data.minSeverity`.
