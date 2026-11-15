# Changes

## Unreleased

- RB-080-66: the Vue panel loads in its sandboxed frame: the kit read `localStorage` while it
  loaded, which throws without `allow-same-origin` ("Uncaught SecurityError"); an in-memory
  storage stands in (`web/src/storage-shim.ts`), the sandbox is unchanged.
- RB-080-31: the Vue panel has tabs - the component tree, every custom inspector the kit's
  plugins register (Pinia's stores with state and getters, vue-router's routes), the router's
  current route and routes, and the kit's timeline (mouse, keyboard, component, Pinia and router
  events, recorded while the tab is open; the page's own timeline switches are put back on
  leave). Reading and selecting only. Setting `framework-devtools.vue.timeline` (on): off hides
  the Timeline tab and records nothing. The probe's report is checked with the agent's one
  validator.

## 0.1.0

First release (RB-080-24): the Vue and React DevTools as panels of rBrowser for the active view.
The backends - `react-devtools-inline` 8.0.0 and `@vue/devtools-kit` 8.2.1, both MIT - are
main-world scripts the core registers before the page's scripts (Settings > DevTools & Debugging
> Framework DevTools: `auto` offers them once a page runs the framework, `on` always, `off`
never); the page script relays their DOM-event channel to the panels over the plugin bridge, so
no port and no socket is opened. The React panel is the React DevTools frontend (components,
props, hooks, the profiler); the Vue panel is a component tree with the selected component's
state over the kit's RPC, highlighting the component in the page. Commands
`plugin.framework_devtools.vue` (`Ctrl+Shift+Space D U`) and `plugin.framework_devtools.react`
(`Ctrl+Shift+Space D A`) open the panels; MCP `fwdevtools_active_view` and `fwdevtools_send`
(the panels' channel, over MCP only with `mcp.allowEvaluate`).
