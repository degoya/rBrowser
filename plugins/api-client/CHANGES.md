# Changes

## Unreleased

- Changed (RB-096-11): the header (method, URL, Send; title, collection, Save, Save as new,
  Delete, Copy as code, Benchmark) stands fixed above the request's tabs; the side column is tabs
  Requests, Variables, Import, Export (no import window, no export block), the environment
  picker always shown. Collections are made, renamed, duplicated, exported and deleted, requests
  opened, duplicated, renamed, moved and deleted, from a context menu (also the menu key and
  Shift+F10). The Variables tab edits `.rbrowser/api/http-client.env.json`, private rows going to
  the private values; the request's variables show their values, unknown ones marked; a
  `{{$secret name}}` without a value is set from the panel before the send. Texts that name a
  setting link to it.
- Added (RB-096-08): a request the network panel opened comes into the editor, unsaved, with
  a note of the variables still without a value; the setting `captureSecrets` (private values
  or none) for the values of the secrets of captured requests.
- Added (RB-096-01): the API client - a panel on Nuxt UI to build a request (method, URL with its
  query as rows, headers, body, auth, timeout, redirects, TLS check), send it from rBrowser's
  core and read the answer (status, phases, size, JSON tree with search and JSONPath, source,
  image, headers, cookies); requests kept as `.http` files below `.rbrowser/api/`; the first
  send to a new host asks. MCP `api_collections`, `api_run`, `api_history`; commands `panel`
  (`Ctrl+Shift+Space P 4`) and `send` (`Ctrl+Shift+Space D Y`).
- Added (RB-096-04): checks per request (status, JSONPath equals/contains, header, time) kept as
  `# @rb-check` lines; a collection run with a report whose failures appear in the findings
  panel; a benchmark of a saved request in one or two environments (min, median, p95, max per
  phase). MCP `api_run_collection`, `api_report`, `api_bench`; command `run_collection`
  (`Ctrl+Shift+Space K 1`); settings `benchRepeats`, `benchConcurrency`.
- Added (RB-096-03): "Copy as code" - curl, HTTPie, fetch, a Playwright test - and the export of a
  folder as a Postman v2.1 collection, a `.http` file or a curl script; MCP `api_code`; settings
  `codeShell` and `codeVariables`.
- Added (RB-096-02): Import a collection - Postman, Insomnia (JSON, YAML), Thunder Client, Bruno,
  HTTPie, `.http` files, OpenAPI/Swagger, HAR and a pasted `curl` into `.rbrowser/api/`, with a
  preview, a report of what was taken, reported and left out, and every literal secret as a
  `{{$secret <name>}}` placeholder; the setting `importEnvironments`.
