use std::cell::Cell;
use std::sync::{Arc, Weak};

use super::shared_or_new;

/// A holder of the buffers: its window, and which collector it started.
struct Logs {
    window: u32,
    collector: usize,
}

/// RB-090-34: one collector per window - MCP and the plugins share one store while either holds
/// it, and a new one starts only when the last holder let go or another window asks.
#[test]
fn one_collector_per_window_while_anyone_holds_the_buffers() {
    let started = Cell::new(0);
    let mut slot: Weak<Logs> = Weak::new();
    let mut acquire = |window: u32| {
        shared_or_new(
            &mut slot,
            |logs| logs.window == window,
            || {
                started.set(started.get() + 1);
                Logs {
                    window,
                    collector: started.get(),
                }
            },
        )
    };
    let mcp = acquire(1);
    let plugins = acquire(1);
    assert!(Arc::ptr_eq(&mcp, &plugins), "one store, not two");
    assert_eq!(started.get(), 1, "one collector");
    drop(mcp);
    let again = acquire(1);
    assert!(
        Arc::ptr_eq(&again, &plugins),
        "held by the plugins: still the one"
    );
    assert_eq!(started.get(), 1);
    drop((again, plugins));
    let fresh = acquire(1);
    assert_eq!(fresh.collector, 2, "nobody held it: a new collector");
    let other = acquire(2);
    assert_eq!(
        (other.window, started.get()),
        (2, 3),
        "another window, its own"
    );
}
