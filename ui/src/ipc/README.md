# Shell IPC

`protocol.ts` is the TypeScript mirror of `crates/rb-ipc` - the messages between the shell UI and
the Rust core (contract 0.1). It holds
types only; the client that sends and receives them is RB-010-07's.

- **Shell -> core:** `window.cefQuery({ request: JSON.stringify(msg), onSuccess, onFailure })`
  with a `ShellToCore`. The answer is a JSON `CoreToShell` (`{ "type": "ack" }` when there is
  nothing to return) or `onFailure(code, message)` with an `IpcErrorCode`.
- **Core -> shell:** `window.addEventListener('rb-ipc', (e) => e.detail)`, where `detail` is a
  `CoreToShell` (state pushes, notifications, trust prompts).

Both files change only through the coordinator, together with the Rust types; the JSON shape is
pinned by `crates/rb-ipc/src/lib_tests.rs`.
