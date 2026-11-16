use super::*;

#[test]
fn the_script_embeds_axe_the_runner_and_the_options() {
    let options = RunOptions::new(
        vec!["image-alt".to_owned()],
        &["main".to_owned()],
        &[],
        true,
    );
    let text = script(&options);
    assert!(text.starts_with("(() => {\n"));
    assert!(text.ends_with("\n})()"));
    assert!(text.contains(AXE_SOURCE));
    assert!(text.contains(RUNNER_SOURCE));
    assert!(text.contains(
        r#"return rbA11yAxe.start({"rules":["image-alt"],"include":["main"],"exclude":[],"incomplete":true,"maxNodes":100,"maxTotal":2000,"maxText":300});"#
    ));
}

#[test]
fn the_bundled_axe_is_the_pinned_version() {
    assert!(AXE_SOURCE.starts_with("/*! axe v4.14.0"));
    assert!(AXE_SOURCE.len() > 100_000);
    assert!(RUNNER_SOURCE.contains("rbA11yAxe"));
}

#[test]
fn a_selector_cannot_leave_its_string() {
    let options = RunOptions::new(
        Vec::new(),
        &["\"]);alert(1);//</script>".to_owned()],
        &[],
        false,
    );
    let text = script(&options);
    assert!(text.contains(r#""include":["\"]);alert(1);//</script>"]"#));
}
