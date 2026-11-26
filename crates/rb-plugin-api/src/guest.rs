//! The guest half every first-party plugin wrote the same way (RB-094-11): the failure, severity
//! and finding conversions between `wit-bindgen`'s records and this crate's, and the
//! [`crate::PluginHost`] a component hands its native logic, as one macro, [`crate::guest_host!`].
//!
//! The bindings themselves are the component's: `wit_bindgen::generate!` writes them into the
//! plugin's `src/guest.rs`, so the macro expands there and names them by their generated paths
//! (`rbrowser::plugin::types`, `rbrowser::plugin::views`, ...). What the plugin may call is its
//! manifest's capabilities; the macro wires exactly the interfaces it is told the manifest grants
//! and refuses the others with `plugins.error.capability_denied`, as the real host would, so a
//! component never calls - and never imports - an interface it may not.

/// The host of a plugin component and the conversions its exports use.
///
/// Expands, in the module that holds the `wit_bindgen::generate!` output:
///
/// - `struct <Name>;` with the attributes given (a doc comment), implementing
///   [`crate::PluginHost`]: `host` always; each interface named in `grants` through its
///   binding; every other one refused with [`crate::Capability::denied`] (`settings` answers
///   `None`; `project-write`, `project-range` and `out-stream` keep the trait's refusal).
/// - `from_wit`, `to_wit`, `severity_to_wit` and `finding_to_wit`, the conversions the plugin's
///   `Guest` implementations use (`.map_err(to_wit)`).
///
/// The grants, in any order: `settings` (the manifest declares a settings schema), `views`,
/// `evaluate`, `cdp`, `http`, `project` (`read` and `list`), `project_read` (`read` alone: a
/// plugin that reads known files and never lists keeps the `list` import out), `project_range`
/// (`project-range.read`, RB-099-24), `project_write`, `out`, `out_stream` (`out-stream`'s
/// `begin`, `append` and `commit`, RB-099-24), `findings`, `sidecar`. Anything else is a compile
/// error.
///
/// ```ignore
/// wit_bindgen::generate!({ /* the plugin's world */ generate_all });
///
/// rb_plugin_api::guest_host! {
///     /// The host as the plugin sees it.
///     struct GuestHost grants [settings, views, evaluate, http, out, findings]
/// }
/// ```
#[macro_export]
macro_rules! guest_host {
    (
        $(#[$meta:meta])*
        $vis:vis struct $host:ident grants [$($grant:ident),* $(,)?]
    ) => {
        $crate::__guest_known!($($grant)*);

        $(#[$meta])*
        $vis struct $host;

        $crate::__guest_conversions!();

        impl $crate::PluginHost for $host {
            fn log(&mut self, level: &str, message: &str) {
                rbrowser::plugin::host::log(level, message);
            }

            fn now_unix_seconds(&mut self) -> u64 {
                rbrowser::plugin::host::now_unix_seconds()
            }

            fn report_progress(&mut self, done: u32, total: u32) {
                rbrowser::plugin::host::report_progress(done, total);
            }

            $crate::__guest_grant!(setting [$($grant)*]);
            $crate::__guest_grant!(views [$($grant)*]);
            $crate::__guest_grant!(evaluate [$($grant)*]);
            $crate::__guest_grant!(cdp_send [$($grant)*]);
            $crate::__guest_grant!(fetch [$($grant)*]);
            $crate::__guest_grant!(project_read [$($grant)*]);
            $crate::__guest_grant!(project_list [$($grant)*]);
            $crate::__guest_grant!(project_range [$($grant)*]);
            $crate::__guest_grant!(project_write [$($grant)*]);
            $crate::__guest_grant!(out_write [$($grant)*]);
            $crate::__guest_grant!(out_stream [$($grant)*]);
            $crate::__guest_grant!(report_findings [$($grant)*]);
            $crate::__guest_grant!(sidecar_call [$($grant)*]);
        }
    };
}

/// Refuses a grant [`crate::guest_host!`] does not know; part of that macro.
#[doc(hidden)]
#[macro_export]
macro_rules! __guest_known {
    () => {};
    (settings $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (views $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (evaluate $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (cdp $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (http $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (project $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (project_read $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (project_range $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (project_write $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (out $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (out_stream $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (findings $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    (sidecar $($rest:ident)*) => { $crate::__guest_known!($($rest)*); };
    ($other:ident $($rest:ident)*) => {
        compile_error!(concat!(
            "guest_host!: unknown grant `",
            stringify!($other),
            "` (settings, views, evaluate, cdp, http, project, project_read, project_range, \
             project_write, out, out_stream, findings, sidecar)"
        ));
    };
}

/// The conversions between `wit-bindgen`'s records and the contract's; part of
/// [`crate::guest_host!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __guest_conversions {
    () => {
        #[allow(dead_code, reason = "not every plugin converts in both directions")]
        fn from_wit(failure: rbrowser::plugin::types::Failure) -> $crate::Failure {
            let kind = match failure.kind {
                rbrowser::plugin::types::FailureKind::NotFound => $crate::FailureKind::NotFound,
                rbrowser::plugin::types::FailureKind::AccessDenied => {
                    $crate::FailureKind::AccessDenied
                }
                rbrowser::plugin::types::FailureKind::NotSupported => {
                    $crate::FailureKind::NotSupported
                }
                rbrowser::plugin::types::FailureKind::BadData => $crate::FailureKind::BadData,
                rbrowser::plugin::types::FailureKind::Cancelled => $crate::FailureKind::Cancelled,
                rbrowser::plugin::types::FailureKind::Io => $crate::FailureKind::Io,
                rbrowser::plugin::types::FailureKind::Limit => $crate::FailureKind::Limit,
                rbrowser::plugin::types::FailureKind::Other => $crate::FailureKind::Other,
            };
            $crate::Failure {
                kind,
                code: failure.code,
                params: failure.params.into_iter().collect(),
            }
        }

        #[allow(dead_code, reason = "not every plugin converts in both directions")]
        fn to_wit(failure: $crate::Failure) -> rbrowser::plugin::types::Failure {
            let kind = match failure.kind {
                $crate::FailureKind::NotFound => rbrowser::plugin::types::FailureKind::NotFound,
                $crate::FailureKind::AccessDenied => {
                    rbrowser::plugin::types::FailureKind::AccessDenied
                }
                $crate::FailureKind::NotSupported => {
                    rbrowser::plugin::types::FailureKind::NotSupported
                }
                $crate::FailureKind::BadData => rbrowser::plugin::types::FailureKind::BadData,
                $crate::FailureKind::Cancelled => rbrowser::plugin::types::FailureKind::Cancelled,
                $crate::FailureKind::Io => rbrowser::plugin::types::FailureKind::Io,
                $crate::FailureKind::Limit => rbrowser::plugin::types::FailureKind::Limit,
                $crate::FailureKind::Other => rbrowser::plugin::types::FailureKind::Other,
            };
            rbrowser::plugin::types::Failure {
                kind,
                code: failure.code,
                params: failure.params.into_iter().collect(),
            }
        }

        #[allow(
            dead_code,
            reason = "only a plugin with rules or findings converts a severity"
        )]
        fn severity_to_wit(severity: $crate::Severity) -> rbrowser::plugin::types::Severity {
            match severity {
                $crate::Severity::Info => rbrowser::plugin::types::Severity::Info,
                $crate::Severity::Warning => rbrowser::plugin::types::Severity::Warning,
                $crate::Severity::Error => rbrowser::plugin::types::Severity::Error,
            }
        }

        #[allow(
            dead_code,
            reason = "only a check or a plugin with `findings` converts one"
        )]
        fn finding_to_wit(finding: $crate::Finding) -> rbrowser::plugin::types::Finding {
            rbrowser::plugin::types::Finding {
                rule: finding.rule,
                severity: severity_to_wit(finding.severity),
                message: finding.message,
                params: finding.params,
                view: finding.view,
                url: finding.url,
                selector: finding.selector,
                path: finding.path,
                data: finding.data,
            }
        }
    };
}

/// One function of [`crate::PluginHost`], wired to its binding when the grants (the bracket)
/// name its interface, refused otherwise; part of [`crate::guest_host!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __guest_grant {
    (setting [settings $($rest:ident)*]) => {
        fn setting(&mut self, key: &str) -> Option<String> {
            rbrowser::plugin::settings::get(key)
        }
    };
    (setting [$other:ident $($rest:ident)*]) => { $crate::__guest_grant!(setting [$($rest)*]); };
    (setting []) => {
        fn setting(&mut self, _key: &str) -> Option<String> {
            None
        }
    };

    (views [views $($rest:ident)*]) => {
        fn views(&mut self) -> Result<Vec<$crate::ViewInfo>, $crate::Failure> {
            Ok(rbrowser::plugin::views::list()
                .into_iter()
                .map(|view| $crate::ViewInfo {
                    id: view.id,
                    label: view.label,
                    breakpoint: view.breakpoint,
                    engine: view.engine,
                    width: view.width,
                    height: view.height,
                    dpr: view.dpr,
                    url: view.url,
                    title: view.title,
                })
                .collect())
        }

        fn active_view(&mut self) -> Result<Option<$crate::ViewId>, $crate::Failure> {
            Ok(rbrowser::plugin::views::active())
        }
    };
    (views [$other:ident $($rest:ident)*]) => { $crate::__guest_grant!(views [$($rest)*]); };
    (views []) => {
        fn views(&mut self) -> Result<Vec<$crate::ViewInfo>, $crate::Failure> {
            Err($crate::Capability::Views.denied())
        }

        fn active_view(&mut self) -> Result<Option<$crate::ViewId>, $crate::Failure> {
            Err($crate::Capability::Views.denied())
        }
    };

    (evaluate [evaluate $($rest:ident)*]) => {
        fn evaluate(
            &mut self,
            view: $crate::ViewId,
            script: &str,
        ) -> Result<String, $crate::Failure> {
            rbrowser::plugin::evaluate::evaluate(view, script).map_err(from_wit)
        }
    };
    (evaluate [$other:ident $($rest:ident)*]) => { $crate::__guest_grant!(evaluate [$($rest)*]); };
    (evaluate []) => {
        fn evaluate(
            &mut self,
            _view: $crate::ViewId,
            _script: &str,
        ) -> Result<String, $crate::Failure> {
            Err($crate::Capability::Evaluate.denied())
        }
    };

    (cdp_send [cdp $($rest:ident)*]) => {
        fn cdp_send(
            &mut self,
            view: $crate::ViewId,
            method: &str,
            params: &str,
        ) -> Result<String, $crate::Failure> {
            rbrowser::plugin::cdp::send(view, method, params).map_err(from_wit)
        }
    };
    (cdp_send [$other:ident $($rest:ident)*]) => { $crate::__guest_grant!(cdp_send [$($rest)*]); };
    (cdp_send []) => {
        fn cdp_send(
            &mut self,
            _view: $crate::ViewId,
            _method: &str,
            _params: &str,
        ) -> Result<String, $crate::Failure> {
            Err($crate::Capability::Cdp.denied())
        }
    };

    (fetch [http $($rest:ident)*]) => {
        fn fetch(
            &mut self,
            request: &$crate::HttpRequest,
        ) -> Result<$crate::HttpResponse, $crate::Failure> {
            rbrowser::plugin::http::fetch(
                &request.method,
                &request.url,
                &request.headers,
                &request.body,
            )
            .map(|response| $crate::HttpResponse {
                status: response.status,
                headers: response.headers,
                body: response.body,
            })
            .map_err(from_wit)
        }
    };
    (fetch [$other:ident $($rest:ident)*]) => { $crate::__guest_grant!(fetch [$($rest)*]); };
    (fetch []) => {
        fn fetch(
            &mut self,
            _request: &$crate::HttpRequest,
        ) -> Result<$crate::HttpResponse, $crate::Failure> {
            Err($crate::Capability::Http.denied())
        }
    };

    (project_read [project $($rest:ident)*]) => {
        $crate::__guest_grant!(project_read [project_read]);
    };
    (project_read [project_read $($rest:ident)*]) => {
        fn project_read(&mut self, path: &str) -> Result<Vec<u8>, $crate::Failure> {
            rbrowser::plugin::project::read(path).map_err(from_wit)
        }
    };
    (project_read [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(project_read [$($rest)*]);
    };
    (project_read []) => {
        fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, $crate::Failure> {
            Err($crate::Capability::ProjectRead.denied())
        }
    };

    (project_list [project $($rest:ident)*]) => {
        fn project_list(&mut self, dir: &str) -> Result<Vec<String>, $crate::Failure> {
            rbrowser::plugin::project::list(dir).map_err(from_wit)
        }
    };
    (project_list [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(project_list [$($rest)*]);
    };
    (project_list []) => {
        fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, $crate::Failure> {
            Err($crate::Capability::ProjectRead.denied())
        }
    };

    (project_range [project_range $($rest:ident)*]) => {
        fn project_read_range(
            &mut self,
            path: &str,
            offset: u64,
            length: u32,
        ) -> Result<(Vec<u8>, u64), $crate::Failure> {
            rbrowser::plugin::project_range::read(path, offset, length).map_err(from_wit)
        }
    };
    (project_range [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(project_range [$($rest)*]);
    };
    // Not granted: the trait's own refusal.
    (project_range []) => {};

    (project_write [project_write $($rest:ident)*]) => {
        fn project_write(
            &mut self,
            path: &str,
            bytes: &[u8],
            expected: Option<&[u8]>,
        ) -> Result<(), $crate::Failure> {
            rbrowser::plugin::project_write::write(path, bytes, expected).map_err(from_wit)
        }
    };
    (project_write [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(project_write [$($rest)*]);
    };
    // Not granted: the trait's own refusal, as a component built before the capability had it.
    (project_write []) => {};

    (out_write [out $($rest:ident)*]) => {
        fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), $crate::Failure> {
            rbrowser::plugin::out::write(path, bytes).map_err(from_wit)
        }
    };
    (out_write [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(out_write [$($rest)*]);
    };
    (out_write []) => {
        fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), $crate::Failure> {
            Err($crate::Capability::OutWrite.denied())
        }
    };

    (out_stream [out_stream $($rest:ident)*]) => {
        fn out_begin(&mut self, path: &str) -> Result<(), $crate::Failure> {
            rbrowser::plugin::out_stream::begin(path).map_err(from_wit)
        }

        fn out_append(&mut self, bytes: &[u8]) -> Result<(), $crate::Failure> {
            rbrowser::plugin::out_stream::append(bytes).map_err(from_wit)
        }

        fn out_commit(&mut self) -> Result<(), $crate::Failure> {
            rbrowser::plugin::out_stream::commit().map_err(from_wit)
        }
    };
    (out_stream [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(out_stream [$($rest)*]);
    };
    // Not granted: the trait's own refusals.
    (out_stream []) => {};

    (report_findings [findings $($rest:ident)*]) => {
        fn report_findings(&mut self, items: &[$crate::Finding]) -> Result<(), $crate::Failure> {
            let items: Vec<rbrowser::plugin::types::Finding> =
                items.iter().cloned().map(finding_to_wit).collect();
            rbrowser::plugin::findings::report(&items).map_err(from_wit)
        }
    };
    (report_findings [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(report_findings [$($rest)*]);
    };
    (report_findings []) => {
        fn report_findings(&mut self, _items: &[$crate::Finding]) -> Result<(), $crate::Failure> {
            Err($crate::Capability::Findings.denied())
        }
    };

    (sidecar_call [sidecar $($rest:ident)*]) => {
        fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, $crate::Failure> {
            rbrowser::plugin::sidecar::call(method, params).map_err(from_wit)
        }
    };
    (sidecar_call [$other:ident $($rest:ident)*]) => {
        $crate::__guest_grant!(sidecar_call [$($rest)*]);
    };
    (sidecar_call []) => {
        fn sidecar_call(
            &mut self,
            _method: &str,
            _params: &str,
        ) -> Result<String, $crate::Failure> {
            Err($crate::Capability::Sidecar.denied())
        }
    };
}

#[cfg(test)]
#[path = "guest_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guest_large_tests.rs"]
mod large_tests;
