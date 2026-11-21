//! Consecutive input into one field is one `fill` with the field's last value (RB-050-01); a
//! placeholder of a secret is never typed onto.

use super::{Builder, Capture, Trail};
use crate::recorder::model::{Action, Locator};

impl Builder {
    /// `value` - else the typed `data` - into the field `target`: the last step when it fills the
    /// same field, in the views both ran in; a new `fill` otherwise.
    pub(super) fn fill(
        &mut self,
        capture: &Capture<'_>,
        target: Locator,
        value: Option<String>,
        data: Option<&str>,
        kind: &'static str,
        secret: bool,
    ) {
        let path = target.css_path().to_owned();
        let at = self.entries.len().saturating_sub(1);
        if let Some(last) = self.last_entry_untouched()
            && let Action::Fill {
                target: last_target,
                value: last_value,
                secret: last_secret,
            } = &mut last.step.action
            && last_target.css_path() == path
        {
            match value {
                Some(value) => {
                    *last_value = value;
                    *last_secret = secret;
                }
                // A placeholder is never typed onto.
                None if *last_secret => {}
                None => last_value.push_str(data.unwrap_or("")),
            }
            last.trail.views.retain(|view| capture.views.contains(view));
            last.trail.add_origin(kind, &path);
            self.touch(at);
            return;
        }
        let value = value.unwrap_or_else(|| data.unwrap_or("").to_owned());
        let mut trail = Trail {
            views: capture.views.clone(),
            ..Trail::default()
        };
        trail.add_origin(kind, &path);
        self.push(
            Action::Fill {
                target,
                value,
                secret,
            },
            capture.view,
            capture.at_ms,
            trail,
        );
    }
}
