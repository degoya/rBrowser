//! Small engine components for the sandbox's tests, written as WebAssembly text and made into
//! components the way `wasm-tools component new` does (as rb-plugin-host's `testbed.rs`), so the
//! limits are tested without the real engine.
//!
//! Memory layout: `run`'s return area at [`RET`] (discriminant byte, then pointer and length at
//! `+4` and `+8`), the return area of an imported `call` at [`SCRATCH`], data from [`DATA`], the
//! bump allocator's heap from [`HEAP`].

use std::fmt::Write as _;

/// Where `run` writes its result.
const RET: u32 = 4096;
/// Where an imported `call` writes its result.
const SCRATCH: u32 = 8192;
/// The first byte of the data segments.
const DATA: u32 = 16384;
/// One byte `counting` writes its count to.
const COUNT_AT: u32 = 12288;
/// The first byte the allocator hands out.
const HEAP: u32 = 32768;

/// `cabi_realloc(old, old_size, align, new_size)`: a bump allocator growing the memory as needed.
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

/// The core import of `host.call`.
const CALL_IMPORT: &str = "(import \"rbrowser:script/host@0.1.0\" \"call\" \
     (func $call (param i32 i32 i32 i32 i32)))";

/// One engine component.
pub(crate) struct Bed {
    data: Vec<(u32, Vec<u8>)>,
    next: u32,
    imports: Vec<&'static str>,
    functions: Vec<&'static str>,
    body: String,
}

impl Bed {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            next: DATA,
            imports: Vec::new(),
            functions: Vec::new(),
            body: String::new(),
        }
    }

    /// `run` answers `ok(text)`.
    pub(crate) fn answering(text: &str) -> Self {
        let mut bed = Self::new();
        let text = bed.data(text.as_bytes());
        bed.body = ok(0, text);
        bed
    }

    /// `run` answers `err(text)`.
    pub(crate) fn failing(text: &str) -> Self {
        let mut bed = Self::new();
        let text = bed.data(text.as_bytes());
        bed.body = ok(1, text);
        bed
    }

    /// `run` loops for ever: only the deadline ends it.
    pub(crate) fn endless() -> Self {
        let mut bed = Self::new();
        bed.body = "(loop $spin (br $spin))\n(i32.const 0)".to_owned();
        bed
    }

    /// `run` recurses until the wasm stack runs out.
    pub(crate) fn recursing() -> Self {
        let mut bed = Self::new();
        bed.functions.push("(func $deep (call $deep))");
        bed.body = "(call $deep)\n(i32.const 0)".to_owned();
        bed
    }

    /// `run` traps as a panic of the engine would (`unreachable`).
    pub(crate) fn crashing() -> Self {
        let mut bed = Self::new();
        bed.body = "(unreachable)".to_owned();
        bed
    }

    /// `run` grows its memory until the limit refuses.
    pub(crate) fn growing() -> Self {
        let mut bed = Self::new();
        bed.body =
            "(loop $grow (drop (memory.grow (i32.const 16))) (br $grow))\n(i32.const 0)".to_owned();
        bed
    }

    /// `run` counts its runs in a global and answers the count as one digit.
    pub(crate) fn counting() -> Self {
        let mut bed = Self::new();
        bed.body = format!(
            "(global.set $count (i32.add (global.get $count) (i32.const 1)))\n\
             (i32.store8 (i32.const {COUNT_AT}) (i32.add (i32.const 48) (global.get $count)))\n{}",
            ok(0, (COUNT_AT, 1))
        );
        bed
    }

    /// `run` calls `tool` with `arguments` `times` times and answers the last call's result as
    /// its own: `ok` for an answer, `err` for a refusal.
    pub(crate) fn calling(tool: &str, arguments: &str, times: u32) -> Self {
        let mut bed = Self::new();
        let tool = bed.data(tool.as_bytes());
        let arguments = bed.data(arguments.as_bytes());
        bed.imports.push(CALL_IMPORT);
        bed.body = format!(
            "(local.set 6 (i32.const {times}))\n\
             (block $done (loop $again\n\
               (br_if $done (i32.eqz (local.get 6)))\n\
               (call $call (i32.const {}) (i32.const {}) (i32.const {}) (i32.const {}) \
                (i32.const {SCRATCH}))\n\
               (local.set 6 (i32.sub (local.get 6) (i32.const 1)))\n\
               (br $again)))\n\
             (i32.store8 (i32.const {RET}) (i32.load8_u (i32.const {SCRATCH})))\n\
             (i32.store (i32.const {}) (i32.load (i32.const {})))\n\
             (i32.store (i32.const {}) (i32.load (i32.const {})))\n\
             (i32.const {RET})",
            tool.0,
            tool.1,
            arguments.0,
            arguments.1,
            RET + 4,
            SCRATCH + 4,
            RET + 8,
            SCRATCH + 8,
        );
        bed
    }

    fn data(&mut self, bytes: &[u8]) -> (u32, u32) {
        let at = self.next;
        let length = u32::try_from(bytes.len()).expect("small data");
        self.next = (at + length + 7) & !7;
        assert!(self.next <= HEAP, "test bed data overlaps the heap");
        self.data.push((at, bytes.to_vec()));
        (at, length)
    }

    fn core_text(&self) -> String {
        let mut text = String::from("(module\n");
        for import in &self.imports {
            let _ = writeln!(text, "{import}");
        }
        text.push_str("(memory (export \"memory\") 1)\n");
        let _ = writeln!(text, "(global $heap (mut i32) (i32.const {HEAP}))");
        text.push_str("(global $count (mut i32) (i32.const 0))\n");
        for (at, bytes) in &self.data {
            let escaped: String = bytes.iter().map(|byte| format!("\\{byte:02x}")).collect();
            let _ = writeln!(text, "(data (i32.const {at}) \"{escaped}\")");
        }
        text.push_str(REALLOC);
        for function in &self.functions {
            let _ = writeln!(text, "{function}");
        }
        // `run(code, args, target)`: three strings, six core parameters, one local.
        let _ = writeln!(
            text,
            "(func (export \"rbrowser:script/engine@0.1.0#run\") \
             (param i32 i32 i32 i32 i32 i32) (result i32) (local i32)\n{})",
            self.body
        );
        text.push_str(")\n");
        text
    }

    /// The component.
    pub(crate) fn build(&self) -> Vec<u8> {
        let mut module = wat::parse_str(self.core_text()).expect("the core module parses");
        let mut resolve = wit_parser::Resolve::default();
        let package = resolve
            .push_str("script.wit", crate::WIT)
            .expect("the contract parses");
        let world = resolve.packages[package].worlds["script"];
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
}

/// `result<string, string>` at [`RET`]: discriminant `case` (0 ok, 1 err) and `pair`.
fn ok(case: u32, (pointer, length): (u32, u32)) -> String {
    format!(
        "(i32.store8 (i32.const {RET}) (i32.const {case}))\n\
         (i32.store (i32.const {}) (i32.const {pointer}))\n\
         (i32.store (i32.const {}) (i32.const {length}))\n\
         (i32.const {RET})",
        RET + 4,
        RET + 8
    )
}

/// A component importing a WASI interface, which the sandbox never links.
pub(crate) fn importing_wasi() -> Vec<u8> {
    wat::parse_str("(component (import \"wasi:filesystem/preopens@0.2.0\" (instance)))")
        .expect("the component parses")
}
