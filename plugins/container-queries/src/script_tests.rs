use super::*;

#[test]
fn the_scripts_call_the_bundle_with_quoted_arguments() {
    assert!(ANALYZE_JS.contains("var __rbContainers"));
    assert!(analyze_script().ends_with("return __rbContainers.analyze();\n})()"));
    let resize = resize_script("#a\"); alert(1); (\"", 400.0);
    assert!(resize.contains(r##"return __rbContainers.resize("#a\"); alert(1); (\"", 400);"##));
    assert!(resize_script("#b", 412.5).contains(r##"resize("#b", 412.5)"##));
    assert!(reset_script(None).contains("return __rbContainers.reset(null);"));
    assert!(reset_script(Some("#hero")).contains(r##"reset("#hero")"##));
}
