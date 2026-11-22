//! rBrowser's script engine (RB-098-01, MCP code mode): one short JavaScript per run on the Boa
//! engine, its only way out the [`Host`] - one MCP tool call at a time and the wall clock.
//!
//! The component (`src/guest.rs`, wasm32 only) exports `rbrowser:script/engine.run` and imports
//! `rbrowser:script/host` (`crates/rb-script/wit/script.wit`) and nothing else: no WASI, so no file
//! system, network, environment or process exists for a script to reach. rBrowser's sandbox
//! (`crates/rb-script`) runs it in wasmtime with a memory limit and an epoch deadline, one
//! instance per run. The logic here is plain Rust and tested natively.
//!
//! A run: a fresh realm, the prelude (`src/prelude.js`: `rb`, `console`), then the script as the
//! body of `async function (args, target)`. `rb.<tool>(input)` calls the host synchronously and
//! answers a promise; a refusal rejects it with an `Error` named `ToolError` carrying `tool` and
//! `code`. The jobs run to the end; a promise that never settles is `script.unsettled`.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use boa_engine::context::Clock;
use boa_engine::context::time::JsInstant;
use boa_engine::module::IdleModuleLoader;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source};

#[cfg(target_arch = "wasm32")]
mod guest;

/// What a script reaches: the MCP tools and the clock.
pub trait Host {
    /// Runs the MCP tool `tool` with `arguments` (JSON text): its answer, or the refusal as JSON
    /// text `{"code", "message"}`.
    ///
    /// # Errors
    ///
    /// The tool's refusal.
    fn call(&mut self, tool: &str, arguments: &str) -> Result<String, String>;

    /// Milliseconds since the Unix epoch.
    fn clock_ms(&self) -> u64;
}

/// The code of a script that does not parse.
pub const SYNTAX: &str = "script.syntax";
/// The code of a script whose promise never settled.
pub const UNSETTLED: &str = "script.unsettled";
/// The code of a failure of the engine itself.
pub const INTERNAL: &str = "script.internal";

/// The prelude every run starts with.
const PRELUDE: &str = include_str!("prelude.js");
/// What comes before the script: its first line is the source's second.
const WRAPPER_HEAD: &str = "__rb_start(async function (args, target) {\n";
/// What comes after the script.
const WRAPPER_TAIL: &str = "\n});\n";

thread_local! {
    /// The host of the run going on; one run at a time per thread (the component has one).
    static HOST: RefCell<Option<Box<dyn Host>>> = RefCell::new(None);
}

/// Runs `code` with `args` and `target` (JSON texts). Ok: `{"value", "logs"}` as JSON text. Err:
/// `{"code", "name", "message", "line"?, "tool"?, "logs"?}` as JSON text.
///
/// # Errors
///
/// The script does not parse (`script.syntax`), threw (`script.error` or the tool's code),
/// never settled (`script.unsettled`), or the engine failed (`script.internal`).
pub fn run(code: &str, args: &str, target: &str, host: Box<dyn Host>) -> Result<String, String> {
    HOST.with(|slot| *slot.borrow_mut() = Some(host));
    let answer = run_in_realm(code, args, target);
    HOST.with(|slot| *slot.borrow_mut() = None);
    answer
}

fn run_in_realm(code: &str, args: &str, target: &str) -> Result<String, String> {
    let mut context = Context::builder()
        .clock(Rc::new(HostClock::default()))
        // No module is ever loaded: `import()` is refused, and no file is read.
        .module_loader(Rc::new(IdleModuleLoader))
        .build()
        .map_err(|error| failure(INTERNAL, "Error", &error.to_string(), None))?;
    prepare(&mut context, args, target)
        .map_err(|error| failure(INTERNAL, "Error", &error.to_string(), None))?;
    let source = format!("{WRAPPER_HEAD}{code}{WRAPPER_TAIL}");
    let parsed = Source::from_bytes(source.as_bytes()).with_path(Path::new("script"));
    if let Err(error) = context.eval(parsed) {
        let message = error.to_string();
        return Err(failure(
            SYNTAX,
            "SyntaxError",
            &message,
            syntax_line(&message),
        ));
    }
    context
        .run_jobs()
        .map_err(|error| failure(INTERNAL, "Error", &error.to_string(), None))?;
    let outcome = context
        .eval(Source::from_bytes("__rb_outcome()"))
        .map_err(|error| failure(INTERNAL, "Error", &error.to_string(), None))?;
    if outcome.is_undefined() {
        return Err(failure(
            UNSETTLED,
            "Error",
            "the script awaited a promise that never settles",
            None,
        ));
    }
    let text = outcome
        .to_string(&mut context)
        .map_err(|error| failure(INTERNAL, "Error", &error.to_string(), None))?
        .to_std_string_lossy();
    match text.split_at_checked(1) {
        Some(("1", rest)) => Ok(rest.to_owned()),
        Some(("0", rest)) => Err(rest.to_owned()),
        _ => Err(failure(INTERNAL, "Error", "the run left no outcome", None)),
    }
}

/// The natives and inputs the prelude takes, then the prelude.
fn prepare(context: &mut Context, args: &str, target: &str) -> JsResult<()> {
    context.register_global_callable(
        JsString::from("__rb_call"),
        2,
        NativeFunction::from_fn_ptr(native_call),
    )?;
    let removable = Attribute::WRITABLE | Attribute::CONFIGURABLE;
    context.register_global_property(
        JsString::from("__rb_args"),
        JsString::from(args),
        removable,
    )?;
    context.register_global_property(
        JsString::from("__rb_target"),
        JsString::from(target),
        removable,
    )?;
    context.eval(Source::from_bytes(PRELUDE).with_path(Path::new("rb")))?;
    Ok(())
}

/// `__rb_call(tool, json)`: `"1" + answer` or `"0" + refusal`.
fn native_call(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let text = |index: usize, context: &mut Context| -> JsResult<String> {
        Ok(args
            .get(index)
            .cloned()
            .unwrap_or_else(JsValue::undefined)
            .to_string(context)?
            .to_std_string_lossy())
    };
    let tool = text(0, context)?;
    let arguments = text(1, context)?;
    let answer = HOST.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map(|host| host.call(&tool, &arguments))
    });
    let flagged = match answer {
        Some(Ok(answer)) => format!("1{answer}"),
        Some(Err(refusal)) => format!("0{refusal}"),
        None => {
            return Err(JsNativeError::typ()
                .with_message("no host for this run")
                .into());
        }
    };
    Ok(JsString::from(flagged.as_str()).into())
}

/// The failure JSON the host reads.
fn failure(code: &str, name: &str, message: &str, line: Option<u32>) -> String {
    let mut body = serde_json::json!({ "code": code, "name": name, "message": message });
    if let Some(line) = line {
        body["line"] = serde_json::json!(line);
    }
    body.to_string()
}

/// The script's line of a parse error (`... at line 3, col 5`), the wrapper's line taken off.
fn syntax_line(message: &str) -> Option<u32> {
    let (_, rest) = message.rsplit_once("at line ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let line: u32 = digits.parse().ok()?;
    line.checked_sub(1).filter(|line| *line > 0)
}

/// The clock of a run: the host's wall clock, never going back for the monotonic half.
#[derive(Default)]
struct HostClock {
    last: std::cell::Cell<u64>,
}

impl HostClock {
    fn now_ms(&self) -> u64 {
        let now = HOST.with(|slot| slot.borrow().as_ref().map_or(0, |host| host.clock_ms()));
        let now = now.max(self.last.get());
        self.last.set(now);
        now
    }
}

impl Clock for HostClock {
    fn now(&self) -> JsInstant {
        let millis = self.now_ms();
        let nanos = u32::try_from((millis % 1000) * 1_000_000).unwrap_or(0);
        JsInstant::new(millis / 1000, nanos)
    }

    fn system_time_millis(&self) -> i64 {
        i64::try_from(self.now_ms()).unwrap_or(i64::MAX)
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
