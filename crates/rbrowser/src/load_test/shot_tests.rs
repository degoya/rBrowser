use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use rb_api_model::Entry;
use serde_json::json;

use super::*;
use crate::api_client::ops::Settings;
use crate::load_test::engine::{End, Halt, OnSecond, Plan, run};
use crate::load_test::gate::Gate;
use crate::load_test::scenario::{AbortRules, Order, Profile, Think};

fn no_env(_name: &str) -> Option<String> {
    None
}

/// The head of each request the server got.
type Heads = std::sync::Arc<Mutex<Vec<String>>>;

fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return None,
        }
    }
    Some(String::from_utf8_lossy(&head).into_owned())
}

/// A local test server answering `200 ok` to every request on kept-alive connections; when
/// `hang`, it reads a request and never answers.
fn server(hang: bool) -> (u16, Heads) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let heads: Heads = Heads::default();
    let noted = std::sync::Arc::clone(&heads);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let noted = std::sync::Arc::clone(&noted);
            thread::spawn(move || {
                while let Some(head) = read_head(&mut stream) {
                    noted.lock().expect("heads").push(head);
                    if hang {
                        thread::sleep(Duration::from_secs(120));
                        return;
                    }
                    let answer = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
                    if stream.write_all(answer).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (port, heads)
}

fn live(root: &std::path::Path, url: &str, users: u32) -> Live {
    let gate = Gate::new(&["https://shop.example.com".to_owned()], &[]);
    let mut request = ApiRequest::new("GET", url);
    for (name, value) in [
        ("User-Agent", "Mozilla/5.0"),
        ("X-Forwarded-For", "10.9.9.9"),
        ("X-User", "{{user}}"),
    ] {
        request.headers.push(Entry {
            name: name.into(),
            value: value.into(),
            enabled: true,
        });
    }
    let scenario = Scenario::from_json(
        r#"{"name":"t","steps":[{"request":"ping.http"}],"variables":{"user":["ada","bob"]},"profile":{"users":2,"holdS":1}}"#,
    )
    .expect("scenario");
    Live {
        ctx: Context {
            root: Some(root.to_path_buf()),
            trusted: true,
            environment: None,
            environments: Vec::new(),
            allowed_hosts: gate.allowed(&["127.0.0.1".to_owned()]),
            secrets: json!({}),
            settings: Settings {
                history_size: 0,
                max_response_bytes: 4096,
                bench_repeats: 1,
                bench_concurrency: 1,
                code: crate::api_client::export::CodeSettings::default(),
            },
            now: 1_700_000_000,
        },
        store: Store::new(root),
        sender: LoadSender::new(users),
        steps: vec![Target {
            id: "ping.http".into(),
            request,
        }],
        scenario,
        environment: None,
        env: &no_env,
    }
}

fn plan(users: u32, hold_s: u32, rps: f64) -> Plan {
    Plan {
        profile: Profile {
            users,
            ramp_up_s: 0,
            hold_s,
            ramp_down_s: 0,
            rps: Some(rps),
        },
        rps: Some(rps),
        order: Order::Sequence,
        weights: vec![1],
        thinks: vec![Think::default()],
        abort: AbortRules::default(),
    }
}

#[test]
fn a_request_goes_out_honestly_named_with_the_users_values() {
    let (port, heads) = server(false);
    let root = tempfile::tempdir().expect("temp");
    let live = live(root.path(), &format!("http://127.0.0.1:{port}/ping"), 2);
    let hit = live.fire_blocking(0, 1, 0);
    assert_eq!(hit.status, Some(200), "{hit:?}");
    assert_eq!(hit.failed, None);
    let heads = heads.lock().expect("heads").clone();
    let head = heads.first().expect("a request").to_ascii_lowercase();
    assert!(head.contains("user-agent: rbrowser/"), "{head}");
    assert!(head.contains("(rbrowser load test)"), "{head}");
    assert!(!head.contains("mozilla"), "{head}");
    assert!(!head.contains("x-forwarded-for"), "{head}");
    assert!(head.contains("x-user: bob"), "{head}");
}

#[test]
fn a_host_outside_the_gate_is_never_sent_to() {
    let root = tempfile::tempdir().expect("temp");
    let live = live(root.path(), "http://example.org/ping", 1);
    let hit = live.fire_blocking(0, 0, 0);
    assert_eq!(hit.failed, Some(Failed::Blocked));
    assert_eq!(hit.status, None);
}

#[test]
fn a_redirect_to_a_host_outside_the_gate_is_not_followed() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_head(&mut stream);
            let answer =
                b"HTTP/1.1 302 Found\r\nLocation: http://example.org/\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(answer);
        }
    });
    let root = tempfile::tempdir().expect("temp");
    let live = live(root.path(), &format!("http://127.0.0.1:{port}/"), 1);
    let hit = live.fire_blocking(0, 0, 0);
    // The redirect's answer is the request's: its Location was not followed.
    assert_eq!(hit.status, Some(302));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_against_a_local_server_counts_every_request() {
    let (port, heads) = server(false);
    let root = tempfile::tempdir().expect("temp");
    let shot = LiveShot(Arc::new(live(
        root.path(),
        &format!("http://127.0.0.1:{port}/ping"),
        2,
    )));
    let on_second: OnSecond = Box::new(|_, _| {});
    let finished = tokio::time::timeout(
        Duration::from_secs(60),
        run(plan(2, 1, 20.0), Arc::new(shot), Halt::default(), on_second),
    )
    .await
    .expect("the run ends");
    assert_eq!(finished.end, End::Completed);
    let totals = finished.recorder.totals(finished.elapsed_s);
    assert!(totals.requests >= 5, "{totals:?}");
    assert_eq!(totals.errors, 0, "{:?}", finished.recorder.errors());
    assert!(heads.lock().expect("heads").len() as u64 >= totals.requests);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_closes_a_connection_that_waits_for_its_answer() {
    let (port, heads) = server(true);
    let root = tempfile::tempdir().expect("temp");
    let live = Arc::new(live(
        root.path(),
        &format!("http://127.0.0.1:{port}/slow"),
        1,
    ));
    let firing = Arc::clone(&live);
    let waiting = tokio::task::spawn_blocking(move || firing.fire_blocking(0, 0, 0));
    // Until the server holds the request.
    let held = tokio::time::timeout(Duration::from_secs(30), async {
        while heads.lock().expect("heads").is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(held.is_ok(), "the request reached the server");
    LiveShot(Arc::clone(&live)).halt();
    // The request's own timeout is 30 s and the server waits 120 s: only the halt ends it here.
    let hit = tokio::time::timeout(Duration::from_secs(20), waiting)
        .await
        .expect("the halt ended the send")
        .expect("joined");
    assert_eq!(hit.status, None);
    assert!(hit.failed.is_some());
}
