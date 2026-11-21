//! The expressions the component evaluates in a view: `assets/analyze.js` (built from
//! `web/src/analyze-entry.ts`, `npm run build` in `web/`) with one call of its global
//! `__rbContainers`. The page script draws with the same code.

/// The bundled analysis: defines `__rbContainers` with `analyze`, `resize` and `reset`.
pub const ANALYZE_JS: &str = include_str!("../assets/analyze.js");

fn call(expression: &str) -> String {
    format!("(() => {{\n{ANALYZE_JS}\nreturn __rbContainers.{expression};\n}})()")
}

/// The view's containers and conditions (`web/src/types.ts`, `Report`).
#[must_use]
pub fn analyze_script() -> String {
    call("analyze()")
}

/// Pins the width of the container at `path` to `width` px; answers whether the page has it.
#[must_use]
pub fn resize_script(path: &str, width: f64) -> String {
    let path = serde_json::Value::String(path.to_owned());
    call(&format!("resize({path}, {width})"))
}

/// Resets the container at `path`, or every container without one; answers how many.
#[must_use]
pub fn reset_script(path: Option<&str>) -> String {
    let path = path.map_or(serde_json::Value::Null, |path| {
        serde_json::Value::String(path.to_owned())
    });
    call(&format!("reset({path})"))
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
