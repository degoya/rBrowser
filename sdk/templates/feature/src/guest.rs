//! The component: `src/lib.rs` translated into the vocabulary of `wit/rbrowser-plugin.wit`. Built
//! only for `wasm32`; it holds no logic of its own, so it needs no test of its own.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "wit",
    world: "feature-plugin",
});

use exports::rbrowser::plugin::feature::Guest;
use rbrowser::plugin::types::{Failure, FailureKind};
use rbrowser::plugin::{host, views};

struct Component;

fn unknown(name: String) -> Failure {
    Failure {
        kind: FailureKind::NotFound,
        code: Some("plugins.error.unknown_contribution".to_owned()),
        params: vec![("name".to_owned(), name)],
    }
}

fn run(name: String) -> Result<String, Failure> {
    if name != "count_views" {
        return Err(unknown(name));
    }
    let list: Vec<crate::View> = views::list()
        .into_iter()
        .map(|view| crate::View {
            id: view.id,
            width: view.width,
        })
        .collect();
    host::log("debug", &format!("{} views", list.len()));
    Ok(crate::count_views(&list))
}

impl Guest for Component {
    fn activate() -> Result<(), Failure> {
        Ok(())
    }

    fn run_command(command: String, _args: String) -> Result<String, Failure> {
        run(command)
    }

    fn call_tool(tool: String, _args: String) -> Result<String, Failure> {
        run(tool)
    }
}

export!(Component);
