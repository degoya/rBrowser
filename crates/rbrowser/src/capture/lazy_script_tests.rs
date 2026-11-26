use serde_json::json;

use super::*;

#[test]
fn a_step_sets_the_echo_marker_before_it_scrolls_instantly() {
    let script = step(Some(Scroll { x: 0.0, y: 852.5 }), 1000).expect("a script");
    let marker = script
        .find(r#"agent.command({"cmd":"replay","ms":1000})"#)
        .expect("the marker");
    let scroll = script
        .find("window.scrollTo({left:0,top:852.5,behavior:'instant'})")
        .expect("the scroll");
    assert!(marker < scroll, "{script}");
    assert!(script.contains(&format!("globalThis.{AGENT_GLOBAL}")));
    assert!(script.contains("requestAnimationFrame"));
}

#[test]
fn a_read_does_not_scroll_and_a_position_is_always_a_number() {
    let read = step(None, 1000).expect("a script");
    assert!(!read.contains("scrollTo"), "{read}");
    assert!(read.contains("scrollHeight"));
    let odd = step(
        Some(Scroll {
            x: f64::NAN,
            y: f64::INFINITY,
        }),
        1000,
    )
    .expect("a script");
    assert!(odd.contains("scrollTo({left:0,top:0,"), "{odd}");
}

#[test]
fn the_image_wait_gives_up_after_its_time() {
    let script = images(Duration::from_millis(2500));
    assert!(
        script.contains("setTimeout(()=>done(true),2500)"),
        "{script}"
    );
    assert!(script.contains("document.images"));
    assert!(script.contains("decode()"));
}

#[test]
fn answers_are_read_and_anything_else_is_refused() {
    let answer = json!({ "x": 0, "y": 120.5, "height": 4000, "viewport": 852 });
    assert_eq!(
        seen(&answer),
        Some(Seen {
            at: Scroll { x: 0.0, y: 120.5 },
            height: 4000.0,
            viewport: 852.0
        })
    );
    assert_eq!(seen(&json!({ "token": 3 })), None);
    assert_eq!(seen(&json!(true)), None);
    assert_eq!(
        seen(&json!({ "x": 0, "y": 0, "height": 10, "viewport": 0 })),
        None
    );
    assert_eq!(pending(&json!({ "pending": 3 })), Some(3));
    assert_eq!(pending(&json!({ "pending": -1 })), None);
    assert_eq!(pending(&json!(null)), None);
}
