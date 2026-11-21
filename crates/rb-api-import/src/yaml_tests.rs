use serde_json::json;

use super::*;

#[test]
fn yaml_reads_as_json() {
    let text = "openapi: 3.0.3\ninfo:\n  title: Pets\n  version: '1'\npaths:\n  /pets:\n    get:\n      tags: [pets]\n      parameters:\n        - name: limit\n          in: query\n          example: 10\n          required: true\n      x-ratio: 1.5\n      x-off: ~\n";
    let value = parse(text).expect("reads");
    assert_eq!(value["openapi"], json!("3.0.3"));
    assert_eq!(value["info"]["version"], json!("1"));
    let get = &value["paths"]["/pets"]["get"];
    assert_eq!(get["tags"], json!(["pets"]));
    assert_eq!(get["parameters"][0]["example"], json!(10));
    assert_eq!(get["parameters"][0]["required"], json!(true));
    assert_eq!(get["x-ratio"], json!(1.5));
    assert_eq!(get["x-off"], json!(null));
}

#[test]
fn an_alias_is_refused_before_the_parse() {
    let text = "a: &x [1, 2]\nb: *x\n";
    assert_eq!(parse(text), Err("YAML aliases are not read".to_owned()));
    assert_eq!(
        check("text: 'a *b'\nlist: [a, b] # *c\nglob: a*b\nnote: Use *bold* here\n"),
        Ok(())
    );
    assert_eq!(
        check("- &anchor 1\n- *anchor\n"),
        Err("YAML aliases are not read".to_owned())
    );
}

#[test]
fn a_broken_document_says_so() {
    assert!(parse("a: [1, 2\n").is_err());
}

#[test]
fn a_document_too_deep_is_refused_before_it_is_built() {
    // About 40 KB of nested block sequences: built, the tree would overflow the stack when it is
    // dropped; the events alone refuse it.
    let deep = format!("{}x\n", "- ".repeat(20_000));
    assert_eq!(
        parse(&deep),
        Err("the YAML document is too deep".to_owned())
    );
    // Flow nesting: refused by the depth or by the scanner's own limit, either before a build.
    let flow = format!("{}{}", "[".repeat(20_000), "]".repeat(20_000));
    assert!(parse(&flow).is_err());
    let fine = format!("{}x\n", "- ".repeat(100));
    assert!(parse(&fine).is_ok());
}
