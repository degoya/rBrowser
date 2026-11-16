# rBrowser check plugin template

A working `check-plugin` (MIT) with two rules: `title.missing` finds views whose page has no
`<title>`, `lang.missing` those whose `<html>` names no language. Copy the folder, rename the
package in `Cargo.toml`, the `id`, the `key_id` and the texts in `manifest.toml` and
`locales/en.json`, then change the logic.

```bash
cargo test                                            # the logic, natively
cargo build --release --target wasm32-unknown-unknown # the core module
wasm-tools component new target/wasm32-unknown-unknown/release/rbrowser_plugin_check_example.wasm \
  -o target/component.wasm                            # the component, no WASI adapter
rb-plugin-pack package . --component target/component.wasm --key <private.pem> \
  --out target/example.rbplugin                       # signed; without --key: development only
rb-plugin-pack conformance target/example.rbplugin --trust <key_id>=<public key>
```

- `src/lib.rs` holds the logic and says what the host promises this kind: `describe` lists the
  rules (title key, severity, shared id, help page), `run` gets the view, the URL and the rules
  switched on, and every finding names its element's `path` - the stable id is made of it.
  `src/lib_tests.rs` tests it without a WebAssembly toolchain.
- `src/guest.rs` translates it into `wit/rbrowser-plugin.wit` and exists on `wasm32` only.
- `wit/rbrowser-plugin.wit` is the contract `rbrowser:plugin@0.1.0`, byte for byte.
- The manifest grants `views` and `evaluate`; a component that imports anything else is refused when it is
  installed.
- `CHANGES.md` carries one `## <version>` section per release.

The full guide is `docs/plugins.md` and the wiki page *Plugin development*.
