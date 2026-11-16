//! A host for the native tests: views, settings, project files, a record of what was written and
//! reported, and a fake sidecar that serves the `lighthouse.*` methods from scripted outcomes -
//! the stored LHR of `tests/fixtures/` in place of a real Lighthouse run.

use std::collections::{BTreeMap, VecDeque};

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::{Value, json};

/// A stored file from `tests/fixtures/`.
pub fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("the fixture exists")
}

/// How a fake Lighthouse job ends.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Running for `polls` polls, then done with this LHR and HTML report.
    Done {
        polls: u32,
        json: String,
        html: String,
    },
    Failed {
        code: i64,
        message: String,
    },
    Cancelled,
    /// Never ends.
    Hangs,
}

#[derive(Debug)]
struct Job {
    outcome: Outcome,
    polls: u32,
}

/// 2026-11-12 10:00:00 UTC.
pub const NOW: u64 = 1_794_477_600;

#[derive(Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    pub settings: BTreeMap<String, String>,
    pub files: BTreeMap<String, Vec<u8>>,
    /// Below `.rbrowser/out/lighthouse/`.
    pub written: BTreeMap<String, Vec<u8>>,
    pub outcomes: VecDeque<Outcome>,
    /// Every sidecar call fails with this one.
    pub sidecar_failure: Option<Failure>,
    /// The characters of one report slice.
    pub slice: usize,
    pub calls: Vec<(String, Value)>,
    pub reported: Vec<Finding>,
    pub findings_unwired: bool,
    pub progress: Vec<(u32, u32)>,
    pub logs: Vec<(String, String)>,
    pub now: u64,
    jobs: BTreeMap<String, Job>,
}

impl FakeHost {
    /// A host with Chromium views `(id, breakpoint, width)` on one page.
    pub fn with_views(views: &[(ViewId, &str, u32)]) -> Self {
        let mut host = Self {
            slice: 64 * 1024,
            now: NOW,
            ..Self::default()
        };
        for (id, breakpoint, width) in views {
            host.views.push(ViewInfo {
                id: *id,
                label: format!("View {id}"),
                breakpoint: (*breakpoint).to_owned(),
                engine: "chromium".to_owned(),
                width: *width,
                height: 800,
                dpr: if *width < 768 { 3.0 } else { 1.0 },
                url: "http://localhost:5173/".to_owned(),
                title: "fixture".to_owned(),
            });
        }
        host
    }

    pub fn with_file(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_owned(), text.as_bytes().to_vec());
        self
    }

    /// A run already on disk: its summary `run.json` with these breakpoints' scores and
    /// metrics, and a report and LHR per breakpoint.
    pub fn with_run(mut self, run: &str, breakpoints: &[(&str, u8)]) -> Self {
        let entries: Vec<Value> = breakpoints
            .iter()
            .map(|(breakpoint, score)| {
                let html = format!("{run}/{breakpoint}.html");
                let lhr = format!("{run}/{breakpoint}.json");
                self.written
                    .insert(html.clone(), b"<html>report</html>".to_vec());
                self.written.insert(lhr.clone(), b"{}".to_vec());
                json!({
                    "breakpoint": breakpoint,
                    "scores": { "performance": score, "seo": 100 },
                    "metrics": { "largest-contentful-paint": f64::from(*score) * 10.0,
                        "cumulative-layout-shift": 0.1 },
                    "report": format!(".rbrowser/out/lighthouse/{html}"),
                    "json": format!(".rbrowser/out/lighthouse/{lhr}"),
                })
            })
            .collect();
        let summary = json!({ "version": 1, "runId": run, "startedAt": NOW,
            "url": "http://localhost:5173/", "breakpoints": entries, "cancelled": false });
        self.written
            .insert(format!("{run}/run.json"), summary.to_string().into_bytes());
        self.written
            .insert("latest.json".to_owned(), summary.to_string().into_bytes());
        self
    }

    pub fn set(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_owned(), value.to_owned());
        self
    }

    /// Each started job ends as the next of `outcomes`.
    pub fn then(mut self, outcome: Outcome) -> Self {
        self.outcomes.push_back(outcome);
        self
    }

    /// The stored mobile LHR as a finished job.
    pub fn done(self) -> Self {
        let json = fixture("lhr-mobile.json");
        self.then(Outcome::Done {
            polls: 1,
            json,
            html: "<html>report</html>".to_owned(),
        })
    }

    /// The methods called, in order.
    pub fn methods(&self) -> Vec<&str> {
        self.calls
            .iter()
            .map(|(method, _)| method.as_str())
            .collect()
    }

    fn serve(&mut self, method: &str, params: &Value) -> Value {
        let job_id = params["job"].as_str().unwrap_or_default().to_owned();
        match method {
            "lighthouse.start" => {
                let id = format!("lh-{}", self.jobs.len() + 1);
                let outcome = self.outcomes.pop_front().unwrap_or(Outcome::Hangs);
                self.jobs.insert(id.clone(), Job { outcome, polls: 0 });
                json!({ "job": id })
            }
            "lighthouse.poll" => {
                let Some(job) = self.jobs.get_mut(&job_id) else {
                    return json!({ "$error": "(-32011) no job" });
                };
                job.polls += 1;
                // A poll waits in the sidecar.
                self.now += 4;
                match &job.outcome {
                    Outcome::Done { polls, json, html } if job.polls > *polls => json!({
                        "job": job_id, "state": "done",
                        "result": { "finalUrl": "http://localhost:5173/", "lighthouseVersion": "13.6.0",
                            "jsonBytes": json.len(), "htmlBytes": html.len() } }),
                    Outcome::Done { .. } | Outcome::Hangs => {
                        json!({ "job": job_id, "state": "running" })
                    }
                    Outcome::Failed { code, message } => json!({
                        "job": job_id, "state": "failed", "error": { "code": code, "message": message } }),
                    Outcome::Cancelled => json!({ "job": job_id, "state": "cancelled" }),
                }
            }
            "lighthouse.report" => {
                let Some(Job {
                    outcome: Outcome::Done { json, html, .. },
                    ..
                }) = self.jobs.get(&job_id)
                else {
                    return json!({ "$error": "(-32013) no report" });
                };
                let text = if params["format"] == "html" {
                    html
                } else {
                    json
                };
                let offset = usize::try_from(params["offset"].as_u64().unwrap_or(0)).unwrap_or(0);
                let mut end = (offset + self.slice).min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                json!({ "text": &text[offset..end], "offset": offset, "total": text.len(),
                    "next": if end < text.len() { json!(end) } else { Value::Null } })
            }
            "lighthouse.release" => json!({ "released": self.jobs.remove(&job_id).is_some() }),
            "lighthouse.cancel" => json!({ "cancelled": 0 }),
            "lighthouse.status" => json!({ "running": null, "queued": 0 }),
            other => json!({ "$error": format!("unknown method {other}") }),
        }
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.now
    }

    fn report_progress(&mut self, done: u32, total: u32) {
        self.progress.push((done, total));
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.views.first().map(|view| view.id))
    }

    fn evaluate(&mut self, _view: ViewId, _script: &str) -> Result<String, Failure> {
        Err(Capability::Evaluate.denied())
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        if let Some(rest) = path.strip_prefix(".rbrowser/out/lighthouse/")
            && let Some(bytes) = self.written.get(rest)
        {
            return Ok(bytes.clone());
        }
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    /// The plugin's out folder as the host lists it: its files and its folders with a `/`.
    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        if dir.trim_end_matches('/') != ".rbrowser/out/lighthouse" {
            return Ok(Vec::new());
        }
        let mut names: Vec<String> = self
            .written
            .keys()
            .map(|path| match path.split_once('/') {
                Some((folder, _)) => format!("{folder}/"),
                None => path.clone(),
            })
            .collect();
        names.dedup();
        Ok(names)
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.written.insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        if self.findings_unwired {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            ));
        }
        self.reported.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        let params: Value = serde_json::from_str(params).expect("the plugin sends JSON");
        self.calls.push((method.to_owned(), params.clone()));
        if let Some(failure) = &self.sidecar_failure {
            return Err(failure.clone());
        }
        let answer = self.serve(method, &params);
        if let Some(reason) = answer["$error"].as_str() {
            return Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)
                .with_param("reason", format!("`{method}` failed {reason}")));
        }
        Ok(answer.to_string())
    }
}
