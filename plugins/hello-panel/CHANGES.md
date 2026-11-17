# Changes

## Unreleased

- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-87): the panel is built on Nuxt UI with plugins/web-common/panel-ui, as the
  plugin SDK's worked example: the greeting in the status line, the pages that said hello as a
  table with their view, a failed call as one sentence.

## 0.1.0

The test plugin of the plugin runtime (RB-060-12): the page script `page/hello.js` says hello to
the component and shows the component's greetings as a badge; the panel `main` lists the pages
that said hello and greets every view; the command `plugin.hello_panel.say_hello`
(`Ctrl+Shift+Space X H`) and the MCP tool `hello_greet` greet the views through posts. Never
shipped.
