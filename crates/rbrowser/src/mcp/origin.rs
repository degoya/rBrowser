//! Whether the command running on this thread is an MCP tool's call (RB-099-25). The backend
//! runs every command an MCP tool asks for through [`as_mcp`] (`CoreBackend::run_command`), so a
//! command a person and MCP share - a check run, `checks.run_all` and `checks_run` - tells a
//! person's ask (its findings shown, a dock folded to its rail opened) from a tool's (answered
//! with its findings, the fold kept). The mark is the calling thread's: a command handler runs
//! on it, the work it hands to other threads is not marked.

use std::cell::Cell;

thread_local! {
    static MCP: Cell<bool> = const { Cell::new(false) };
}

/// Puts the mark back as it was before [`as_mcp`], also when the call unwinds.
struct Restore(bool);

impl Drop for Restore {
    fn drop(&mut self) {
        MCP.with(|mcp| mcp.set(self.0));
    }
}

/// Runs `call` as an MCP tool's call: [`is_mcp`] answers `true` on this thread until it returns.
pub fn as_mcp<T>(call: impl FnOnce() -> T) -> T {
    let _restore = Restore(MCP.with(|mcp| mcp.replace(true)));
    call()
}

/// Whether the command running on this thread is an MCP tool's call; `false` for a person's (a
/// menu, the palette, a shortcut).
#[must_use]
pub fn is_mcp() -> bool {
    MCP.with(Cell::get)
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod tests;
