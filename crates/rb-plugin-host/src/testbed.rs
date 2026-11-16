//! The test bed: small plugin components that work, hang, or reach for what they may not, written
//! as WebAssembly text and made into components the way `wasm-tools component new` does
//! (`wit-component` over the contract in `rb_plugin_api::WIT`), so `cargo test` builds everything
//! it runs and needs no WebAssembly toolchain.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/testbed.rs`. Layout of the core
//! module's memory: the return area of every export at [`RET`], a scratch area for an import's
//! result at [`SCRATCH`], data from [`DATA`], the bump allocator's heap from 32 KiB. A
//! `result<T, failure>` is a discriminant byte at `RET` and the payload from `RET + 4`; a string
//! is (pointer, length).
//!
//! Every text a bed answers names the plugin `sample` (`plugin.sample.*`), the id of
//! [`crate::test_support::ManifestBuilder`]'s default.

use std::fmt::Write as _;

use rb_plugin_types::PluginKind;

/// Where every export writes its result.
pub const RET: u32 = 4096;
/// Where an import writes its result.
pub const SCRATCH: u32 = 8192;
/// The first byte of the data segments.
pub const DATA: u32 = 16384;
/// The first byte the allocator hands out.
const HEAP: u32 = 32768;

/// A bump allocator: `cabi_realloc(old, old_size, align, new_size)` for what the host passes in.
const REALLOC: &str = "(func (export \"cabi_realloc\") (param i32 i32 i32 i32) (result i32)
  (local $ptr i32)
  (local.set $ptr
    (i32.and
      (i32.add (global.get $heap) (i32.sub (local.get 2) (i32.const 1)))
      (i32.sub (i32.const 0) (local.get 2))))
  (global.set $heap (i32.add (local.get $ptr) (local.get 3)))
  (block $fits
    (br_if $fits (i32.le_u (global.get $heap) (i32.mul (memory.size) (i32.const 65536))))
    (drop (memory.grow
      (i32.add
        (i32.div_u (i32.sub (global.get $heap) (i32.mul (memory.size) (i32.const 65536)))
          (i32.const 65536))
        (i32.const 1)))))
  (local.get $ptr))
";

/// One export: interface, function, core parameters, body (leaving the result pointer).
struct Export {
    interface: &'static str,
    function: &'static str,
    params: &'static str,
    body: String,
}

/// One plugin component, built from a body per export.
pub struct Bed {
    world: &'static str,
    data: Vec<(u32, Vec<u8>)>,
    next: u32,
    imports: Vec<String>,
    exports: Vec<Export>,
}

impl Bed {
    fn new(world: &'static str) -> Self {
        Self {
            world,
            data: Vec::new(),
            next: DATA,
            imports: Vec::new(),
            exports: Vec::new(),
        }
    }

    /// A feature plugin that works: `activate` succeeds, `run-command` and `call-tool` answer
    /// `{}`.
    #[must_use]
    pub fn feature() -> Self {
        let mut bed = Self::new(PluginKind::Feature.world());
        let empty = bed.data(b"{}");
        bed.export("feature", "activate", "", ok_unit());
        bed.export(
            "feature",
            "run-command",
            "(param i32 i32 i32 i32)",
            ok_pair(empty),
        );
        bed.export(
            "feature",
            "call-tool",
            "(param i32 i32 i32 i32)",
            ok_pair(empty),
        );
        bed
    }

    /// A check plugin describing the rule `basic` (a warning, shared id `shared-basic`) that
    /// finds nothing.
    #[must_use]
    pub fn check() -> Self {
        let mut bed = Self::new(PluginKind::Check.world());
        let id = bed.data(b"basic");
        let title = bed.data(b"plugin.sample.checks.basic");
        let shared = bed.data(b"shared-basic");
        // rule-info: id, title, severity (u8, padded), shared-id (some), help-url (none).
        let mut info = record(&[id, title]);
        info.extend([1, 0, 0, 0, 1, 0, 0, 0]);
        info.extend(record(&[shared]));
        info.extend([0; 12]);
        let info = bed.data(&info);
        bed.export("check", "describe", "", pair_at(RET, (info.0, 1)));
        // check-context flattens to 9 core values: view (2), url (3), rules (2), args (2).
        bed.export("check", "run", CHECK_CONTEXT_PARAMS, ok_pair((0, 0)));
        bed
    }

    /// [`Bed::check`] whose `run` finds two images of rule `basic`, as warnings without a view:
    /// `img.a` and `img.b` on `https://site.test/`.
    #[must_use]
    pub fn check_finding_two() -> Self {
        let mut bed = Self::check();
        let rule = bed.data(b"basic");
        let message = bed.data(b"plugin.sample.findings.basic");
        let url = bed.data(b"https://site.test/");
        let mut list = Vec::new();
        for (selector, path) in [
            (&b"img.a"[..], &b"html > body > img:nth-of-type(1)"[..]),
            (&b"img.b"[..], &b"html > body > img:nth-of-type(2)"[..]),
        ] {
            let selector = bed.data(selector);
            let path = bed.data(path);
            list.extend(finding_record(rule, message, url, selector, path));
        }
        let list = bed.data(&list);
        bed.body("run", ok_pair((list.0, 2)))
    }

    /// An analyzer that proposes nothing.
    #[must_use]
    pub fn analyzer() -> Self {
        let mut bed = Self::new(PluginKind::Analyzer.world());
        bed.export("analyzer", "analyze", "(param i32 i32)", ok_pair((0, 0)));
        bed
    }

    /// An export plugin writing the format `text` as an empty `out.txt`.
    #[must_use]
    pub fn exporter() -> Self {
        let mut bed = Self::new(PluginKind::Export.world());
        let id = bed.data(b"text");
        let title = bed.data(b"plugin.sample.formats.text");
        let extension = bed.data(b"txt");
        let format = bed.data(&record(&[id, title, extension]));
        let file = bed.data(b"out.txt");
        bed.export("exporter", "formats", "", pair_at(RET, (format.0, 1)));
        let render = format!(
            "(i32.store8 (i32.const {RET}) (i32.const 0))\n{}\n{}\n(i32.const {RET})",
            store_pair(RET + 4, file),
            store_pair(RET + 12, (0, 0))
        );
        bed.export("exporter", "render", "(param i32 i32 i32 i32)", render);
        bed
    }

    /// Places `bytes` in a data segment; answers its pointer and length.
    pub fn data(&mut self, bytes: &[u8]) -> (u32, u32) {
        let at = self.next;
        let length = u32::try_from(bytes.len()).expect("small data");
        self.next = (at + length + 7) & !7;
        assert!(self.next <= HEAP, "test bed data overlaps the heap");
        self.data.push((at, bytes.to_vec()));
        (at, length)
    }

    fn export(
        &mut self,
        interface: &'static str,
        function: &'static str,
        params: &'static str,
        body: String,
    ) {
        self.exports.push(Export {
            interface,
            function,
            params,
            body,
        });
    }

    /// Replaces the body of `function`.
    #[must_use]
    pub fn body(mut self, function: &str, body: impl Into<String>) -> Self {
        let body = body.into();
        for export in &mut self.exports {
            if export.function == function {
                export.body.clone_from(&body);
            }
        }
        self
    }

    /// `function` loops for ever: only fuel or the epoch deadline end it.
    #[must_use]
    pub fn endless(self, function: &str) -> Self {
        self.body(function, "(loop $spin (br $spin))\n(i32.const 0)")
    }

    /// `function` traps (`unreachable`, a panic).
    #[must_use]
    pub fn crashing(self, function: &str) -> Self {
        self.body(function, "(unreachable)")
    }

    /// `function` (`run-command` or `call-tool`) answers its arguments as they came.
    #[must_use]
    pub fn echoing(self, function: &str) -> Self {
        let body = format!(
            "(i32.store8 (i32.const {RET}) (i32.const 0))\n\
             (i32.store (i32.const {}) (local.get 2))\n\
             (i32.store (i32.const {}) (local.get 3))\n\
             (i32.const {RET})",
            RET + 4,
            RET + 8
        );
        self.body(function, body)
    }

    /// `function` (of the feature world) writes `x` to `path` through `out.write` and answers
    /// `"written"` or `"denied"` as JSON.
    #[must_use]
    pub fn writing(mut self, function: &str, path: &str) -> Self {
        let path = self.data(path.as_bytes());
        let byte = self.data(b"x");
        let written = self.data(b"\"written\"");
        let denied = self.data(b"\"denied\"");
        self.imports.push(format!(
            "(import \"{}\" \"write\" (func $write (param i32 i32 i32 i32 i32)))",
            rb_plugin_api::versioned("out")
        ));
        let body = format!(
            "(call $write (i32.const {}) (i32.const {}) (i32.const {}) (i32.const {}) (i32.const {SCRATCH}))\n\
             (if (i32.eqz (i32.load8_u (i32.const {SCRATCH})))\n\
               (then {})\n\
               (else {}))\n\
             (i32.const {RET})",
            path.0,
            path.1,
            byte.0,
            byte.1,
            store_ok_pair(written),
            store_ok_pair(denied),
        );
        self.body(function, body)
    }

    /// The core module as text.
    #[must_use]
    pub fn core_text(&self) -> String {
        let mut text = String::from("(module\n");
        for import in &self.imports {
            let _ = writeln!(text, "{import}");
        }
        text.push_str("(memory (export \"memory\") 1)\n");
        let _ = writeln!(text, "(global $heap (mut i32) (i32.const {HEAP}))");
        for (at, bytes) in &self.data {
            let escaped: String = bytes.iter().map(|byte| format!("\\{byte:02x}")).collect();
            let _ = writeln!(text, "(data (i32.const {at}) \"{escaped}\")");
        }
        text.push_str(REALLOC);
        for export in &self.exports {
            let _ = writeln!(
                text,
                "(func (export \"{}#{}\") {} (result i32)\n{})",
                rb_plugin_api::versioned(export.interface),
                export.function,
                export.params,
                export.body
            );
        }
        text.push_str(")\n");
        text
    }

    /// The component.
    #[must_use]
    pub fn build(&self) -> Vec<u8> {
        component_of(&self.core_text(), self.world)
    }
}

/// The core parameters of `check.run(ctx: check-context)`.
const CHECK_CONTEXT_PARAMS: &str = "(param i32 i32 i32 i32 i32 i32 i32 i32 i32)";

/// One `types.finding` (84 bytes): rule, severity `warning`, message, no params, no view, `url`,
/// `selector` and `path` set, no data.
fn finding_record(
    rule: (u32, u32),
    message: (u32, u32),
    url: (u32, u32),
    selector: (u32, u32),
    path: (u32, u32),
) -> Vec<u8> {
    let some = |field: (u32, u32)| {
        let mut bytes = vec![1, 0, 0, 0];
        bytes.extend(record(&[field]));
        bytes
    };
    let mut bytes = record(&[rule]);
    bytes.extend([1, 0, 0, 0]);
    bytes.extend(record(&[message, (0, 0)]));
    bytes.extend([0; 8]);
    bytes.extend(some(url));
    bytes.extend(some(selector));
    bytes.extend(some(path));
    bytes.extend([0; 12]);
    debug_assert_eq!(bytes.len(), 84);
    bytes
}

/// A record of string fields, each (pointer, length), as little-endian bytes.
fn record(fields: &[(u32, u32)]) -> Vec<u8> {
    fields
        .iter()
        .flat_map(|(at, length)| at.to_le_bytes().into_iter().chain(length.to_le_bytes()))
        .collect()
}

fn store_pair(at: u32, (pointer, length): (u32, u32)) -> String {
    format!(
        "(i32.store (i32.const {at}) (i32.const {pointer}))\n(i32.store (i32.const {}) (i32.const {length}))",
        at + 4
    )
}

/// A (pointer, length) pair at `at`, answered as the result pointer.
fn pair_at(at: u32, pair: (u32, u32)) -> String {
    format!("{}\n(i32.const {at})", store_pair(at, pair))
}

fn store_ok_pair(pair: (u32, u32)) -> String {
    format!(
        "(i32.store8 (i32.const {RET}) (i32.const 0))\n{}",
        store_pair(RET + 4, pair)
    )
}

/// `ok` of a `result<_, failure>`.
fn ok_unit() -> String {
    format!("(i32.store8 (i32.const {RET}) (i32.const 0))\n(i32.const {RET})")
}

/// `ok` of a `result<string or list, failure>`.
fn ok_pair(pair: (u32, u32)) -> String {
    format!("{}\n(i32.const {RET})", store_ok_pair(pair))
}

/// Makes a component of a core module implementing `world` of the current contract.
fn component_of(core_text: &str, world: &str) -> Vec<u8> {
    let mut module = wat::parse_str(core_text).expect("the core module parses");
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str("rbrowser-plugin.wit", rb_plugin_api::WIT)
        .expect("the contract parses");
    let world = resolve.packages[package].worlds[world];
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
        false,
    )
    .expect("the contract embeds");
    let mut encoder = wit_component::ComponentEncoder::default();
    encoder
        .module(&module)
        .expect("the core module fits the world");
    encoder.validate(true);
    encoder.encode().expect("a valid component")
}

/// A component importing `http`, which only the capability `http` grants.
#[must_use]
pub fn importing_http() -> Vec<u8> {
    wat::parse_str(format!(
        "(component (import \"{}\" (instance (export \"fetch\" (func)))))",
        rb_plugin_api::versioned("http")
    ))
    .expect("the component parses")
}

/// A component importing a WASI interface, which nothing grants.
#[must_use]
pub fn importing_wasi() -> Vec<u8> {
    wat::parse_str("(component (import \"wasi:cli/environment@0.2.0\" (instance)))")
        .expect("the component parses")
}
