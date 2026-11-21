use rb_sync::{AGENT_GLOBAL, BINDING_NAME, BOOT_GLOBAL};

use super::*;

#[test]
fn the_boot_script_runs_the_bundle_in_a_private_scope_once() {
    let config = follower_config();
    assert!(!config.active);
    let script = boot_script("globalThis.__rbAgent = {};", &config, "__rbabc").expect("a bundle");
    assert!(script.contains("const name = \"__rbabc\";"), "{script}");
    // Booted already: nothing runs twice.
    assert!(script.contains("hasOwnProperty.call(globalThis, name)"));
    assert!(script.contains(&format!("{BINDING_NAME}: () => {{}}")));
    assert!(script.contains(&format!("{BOOT_GLOBAL}: {{")));
    assert!(script.contains(
        "(function (globalThis) {\nglobalThis.__rbAgent = {};\n}).call(undefined, scope);"
    ));
    assert!(script.contains(&format!("scope.{AGENT_GLOBAL}")));
    assert!(script.contains("enumerable: false"));
    assert!(script.ends_with("return true;"));
}

#[test]
fn no_bundle_boots_nothing() {
    assert_eq!(boot_script("  ", &follower_config(), "__rbx"), None);
}

#[test]
fn a_name_cannot_break_out_of_the_script() {
    let script = boot_script("x", &follower_config(), "\";alert(1);//").expect("a bundle");
    assert!(
        script.contains(r#"const name = "\";alert(1);//";"#),
        "{script}"
    );
}

#[test]
fn the_command_script_takes_its_name_and_command_as_arguments() {
    assert!(COMMAND_SCRIPT.contains("globalThis[arguments[0]]"));
    assert!(COMMAND_SCRIPT.contains("handle.command(arguments[1])"));
    assert!(COMMAND_SCRIPT.contains("return null;"));
}

#[test]
fn random_names_differ_and_look_like_the_agent_s() {
    let first = random_name();
    let second = random_name();
    assert_ne!(first, second);
    assert!(first.starts_with("__rb"));
    assert_eq!(first.len(), 4 + 32);
    assert!(first[4..].bytes().all(|byte| byte.is_ascii_hexdigit()));
}
