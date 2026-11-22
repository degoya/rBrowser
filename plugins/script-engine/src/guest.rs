//! The component: [`crate::run`] behind `rbrowser:script/engine`, the imports of
//! `rbrowser:script/host` as the [`crate::Host`]. Built only for `wasm32`; no logic of its own
//! beyond the randomness Boa's `Math.random` asks for.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates and getrandom's custom backend symbol"
)]

use std::sync::atomic::{AtomicU64, Ordering};

wit_bindgen::generate!({
    path: "../../crates/rb-script/wit",
    world: "script",
});

use exports::rbrowser::script::engine::Guest;
use rbrowser::script::host;

/// The imports as the engine's host.
struct ImportedHost;

impl crate::Host for ImportedHost {
    fn call(&mut self, tool: &str, arguments: &str) -> Result<String, String> {
        host::call(tool, arguments)
    }

    fn clock_ms(&self) -> u64 {
        host::clock_ms()
    }
}

struct Component;

impl Guest for Component {
    fn run(code: String, args: String, target: String) -> Result<String, String> {
        crate::run(&code, &args, &target, Box::new(ImportedHost))
    }
}

export!(Component);

/// The generator's state; seeded from the clock at the first draw.
static STATE: AtomicU64 = AtomicU64::new(0);

/// One splitmix64 step: `Math.random` needs no secret, and the component has no other source.
fn next() -> u64 {
    let mut state = STATE.load(Ordering::Relaxed);
    if state == 0 {
        state = host::clock_ms() | 1;
    }
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    STATE.store(state, Ordering::Relaxed);
    let mut mixed = state;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// getrandom's custom backend (`getrandom_backend = "custom"`, `.cargo/config.toml`).
///
/// # Safety
///
/// getrandom calls it with a valid, writable buffer of `len` bytes.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    // SAFETY: getrandom passes a buffer it owns, valid for `len` writable bytes.
    let buffer = unsafe { std::slice::from_raw_parts_mut(dest, len) };
    for chunk in buffer.chunks_mut(8) {
        let bytes = next().to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    Ok(())
}
