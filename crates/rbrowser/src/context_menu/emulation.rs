//! The Emulation submenu of the page's context menu (RB-080-49): the toolbar's switches for the
//! clicked view - colour scheme (one of light, dark, no override), reduced motion, offline,
//! location, time zone, text direction and pseudo-localisation (RB-092-02), reset - each the
//! toolbar's own command with `target`, the active state marked; "All views" repeats the
//! switches the toolbar's Shift-click has, with `target: "all"`, marked by what every view has.
//! No logic of its own: the commands are `emulation.set`, `emulation.reset_view`,
//! `view.emulate.*` and `overlays.pseudo_locale`.
//! location, time zone, reset - each the toolbar's own command with `target`, the active state
//! marked; "All views" repeats the switches the toolbar's Shift-click has, with
//! `target: "all"`, marked by what every view has. No logic of its own: the
//! commands are `emulation.set`, `emulation.reset_view` and `view.emulate.*`; the Clock and
//! Animations submenus are `super::time_control`'s (RB-094-07).

use rb_config::{
    ColorScheme, EmulationUserSettings, TIMEZONES, TextDirection, geolocation_presets,
};
use rb_engine::Emulation;
use serde_json::{Value, json};

use super::model::{Action, Builder, Item, Label, Mark};
use crate::emulation::extras::{CPU, GEOLOCATION, OFFLINE, TIMEZONE};
use crate::emulation::page_locale::PSEUDO_LOCALE;

/// The CPU throttling factors offered: the toolbar's (`CPU_RATES`, `ui/src/emulation/extras.ts`).
pub const CPU_RATES: [u32; 4] = [2, 4, 6, 20];

/// A geolocation preset the submenu offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    /// The preset's id (`view.emulate.geolocation { preset }`).
    pub id: String,
    /// The user's name for it; a built-in preset has none (its name is in the catalogue).
    pub label: Option<String>,
}

/// What every view has in common, for "All views": an entry is marked only when all agree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllViews {
    /// Every view has this colour scheme (`Some(None)`: none emulates one); `None` when they
    /// differ or there is no view.
    pub color_scheme: Option<Option<ColorScheme>>,
    /// Every view emulates reduced motion.
    pub reduced_motion: bool,
    /// Every view runs right to left (RB-092-02).
    pub rtl: bool,
    /// Every view is pseudo-localised (RB-092-02; the sync agents know, [`EmulationState::of`]
    /// leaves it off).
    pub pseudo: bool,
}

impl AllViews {
    /// What `views` have in common.
    #[must_use]
    pub fn of(views: &[Emulation]) -> Self {
        let first = views.first().map(|view| view.color_scheme);
        Self {
            color_scheme: first
                .filter(|scheme| views.iter().all(|view| view.color_scheme == *scheme)),
            reduced_motion: !views.is_empty()
                && views.iter().all(|view| view.reduced_motion == Some(true)),
            rtl: !views.is_empty()
                && views
                    .iter()
                    .all(|view| view.dir == Some(TextDirection::Rtl)),
            pseudo: false,
        }
    }
}

/// What the submenu shows of the clicked view's emulation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmulationState {
    /// `prefers-color-scheme`, when emulated.
    pub color_scheme: Option<ColorScheme>,
    /// `prefers-reduced-motion: reduce` is emulated.
    pub reduced_motion: bool,
    /// The view is offline.
    pub offline: bool,
    /// A position is emulated.
    pub located: bool,
    /// The preset the position is, if it is one.
    pub preset: Option<String>,
    /// The emulated time zone.
    pub timezone: Option<String>,
    /// The CPU throttling factor in hundredths; `None` at full speed (a factor of 1 or none).
    pub cpu: Option<u32>,
    /// The presets offered: the built-in ones, then the user's.
    pub presets: Vec<Preset>,
    /// The forced text direction of the page (RB-092-02).
    pub dir: Option<TextDirection>,
    /// The page is pseudo-localised (RB-092-02; the sync agents know, [`Self::of`] leaves it
    /// off).
    pub pseudo: bool,
    /// What all views have in common.
    pub all: AllViews,
    /// The views' clock and the clicked view's animations (RB-094-07); the real time and 100 %
    /// until the menu's state fills it.
    pub time: super::time_control::TimeMenu,
}

impl EmulationState {
    /// The state of `emulation` (the clicked view's; `None`: nothing emulated) among `views`
    /// (every view's emulation), with the presets of `settings`.
    #[must_use]
    pub fn of(
        emulation: Option<&Emulation>,
        views: &[Emulation],
        settings: &EmulationUserSettings,
    ) -> Self {
        let presets = geolocation_presets(settings);
        let position = emulation.and_then(|emulation| emulation.geolocation.as_ref());
        let preset = position.and_then(|position| {
            presets
                .iter()
                .find(|preset| {
                    preset.position.latitude == position.latitude
                        && preset.position.longitude == position.longitude
                })
                .map(|preset| preset.id.clone())
        });
        Self {
            color_scheme: emulation.and_then(|emulation| emulation.color_scheme),
            reduced_motion: emulation
                .is_some_and(|emulation| emulation.reduced_motion == Some(true)),
            offline: emulation
                .and_then(|emulation| emulation.network.as_ref())
                .is_some_and(|network| network.offline),
            located: position.is_some(),
            preset,
            timezone: emulation
                .and_then(|emulation| emulation.timezone.clone())
                .filter(|timezone| !timezone.is_empty()),
            cpu: emulation
                .and_then(|emulation| emulation.cpu_throttling)
                .filter(|rate| rate.is_finite() && *rate > 1.0)
                .map(hundredths),
            presets: presets
                .into_iter()
                .map(|preset| Preset {
                    id: preset.id,
                    label: preset.label,
                })
                .collect(),
            dir: emulation.and_then(|emulation| emulation.dir),
            pseudo: false,
            all: AllViews::of(views),
            time: super::time_control::TimeMenu::default(),
        }
    }
}

/// `rate` in hundredths, for a mark that compares exactly; factors run from 1 to 100.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded and clamped to 0..=10 000 first: the cast neither wraps nor truncates"
)]
fn hundredths(rate: f64) -> u32 {
    (rate * 100.0).round().clamp(0.0, 10_000.0) as u32
}

/// A preset's name: the user's label, else the catalogue's text for the id
/// (`emulation.geolocation.preset.<id>`, the built-in presets), else the id.
fn preset_label(preset: &Preset) -> Label {
    match &preset.label {
        Some(label) => Label::Text(label.clone()),
        None => Label::KeyOr {
            key: format!("emulation.geolocation.preset.{}", preset.id),
            text: preset.id.clone(),
        },
    }
}

impl Builder<'_> {
    /// A marked entry running `command`.
    fn marked(&mut self, label: Label, mark: Mark, command: &str, args: Value) -> Item {
        let action = Action::Run {
            command: command.to_owned(),
            args,
        };
        self.labelled(label, true, mark, action)
    }

    /// `emulation.set` of `fields` for `target`.
    fn set(&mut self, key: &'static str, mark: Mark, target: &Value, fields: Value) -> Item {
        self.marked(
            Label::Key(key),
            mark,
            "emulation.set",
            json!({ "target": target, "fields": fields }),
        )
    }

    /// The colour scheme as one choice of three for `target`: light, dark, or no override
    /// (`colorScheme: null` drops the live override). `scheme` is what is chosen now; `None`
    /// marks nothing (the views differ).
    fn scheme(&mut self, scheme: Option<Option<ColorScheme>>, target: &Value) -> Vec<Item> {
        let chosen = |value: Option<ColorScheme>| Mark::Radio(scheme == Some(value));
        vec![
            self.set(
                "emulation.toolbar.light",
                chosen(Some(ColorScheme::Light)),
                target,
                json!({ "colorScheme": "light" }),
            ),
            self.set(
                "emulation.toolbar.dark",
                chosen(Some(ColorScheme::Dark)),
                target,
                json!({ "colorScheme": "dark" }),
            ),
            self.set(
                "contextmenu.emulation.scheme_none",
                chosen(None),
                target,
                json!({ "colorScheme": Value::Null }),
            ),
        ]
    }

    /// Reduced motion for `target`: checked when `on`, and the choice turns it the other way.
    fn motion(&mut self, on: bool, target: &Value) -> Item {
        self.set(
            "emulation.toolbar.reducedMotion",
            Mark::Check(on),
            target,
            json!({ "reducedMotion": !on }),
        )
    }

    fn reset(&mut self, target: &Value) -> Item {
        self.run(
            "emulation.toolbar.reset",
            true,
            "emulation.reset_view",
            json!({ "target": target }),
        )
    }

    fn location(&mut self, emulation: &EmulationState, view: &Value) -> Item {
        let mut items = vec![self.marked(
            Label::Key("emulation.extras.off"),
            Mark::Radio(!emulation.located),
            GEOLOCATION,
            json!({ "target": view, "geolocation": Value::Null }),
        )];
        for preset in &emulation.presets {
            items.push(self.marked(
                preset_label(preset),
                Mark::Radio(emulation.preset.as_deref() == Some(preset.id.as_str())),
                GEOLOCATION,
                json!({ "target": view, "preset": preset.id }),
            ));
        }
        self.submenu(Label::Key("emulation.extras.geolocation"), items)
    }

    fn timezone(&mut self, emulation: &EmulationState, view: &Value) -> Item {
        let current = emulation.timezone.as_deref();
        let mut items = vec![self.marked(
            Label::Key("emulation.extras.off"),
            Mark::Radio(current.is_none()),
            TIMEZONE,
            json!({ "target": view, "timezone": "" }),
        )];
        // A zone set elsewhere (the toolbar's free field, the config) is shown too.
        let other = current.filter(|zone| !TIMEZONES.contains(zone));
        for zone in TIMEZONES.iter().copied().chain(other) {
            items.push(self.marked(
                Label::Text(zone.to_owned()),
                Mark::Radio(current == Some(zone)),
                TIMEZONE,
                json!({ "target": view, "timezone": zone }),
            ));
        }
        self.submenu(Label::Key("emulation.extras.timezone"), items)
    }

    /// CPU throttling: off (factor 1) or one of [`CPU_RATES`]; a factor set elsewhere (the
    /// config, MCP) is shown too, marked.
    fn cpu(&mut self, emulation: &EmulationState, view: &Value) -> Item {
        let mut items = vec![self.marked(
            Label::Key("emulation.extras.off"),
            Mark::Radio(emulation.cpu.is_none()),
            CPU,
            json!({ "target": view, "rate": 1 }),
        )];
        let other = emulation
            .cpu
            .filter(|current| !CPU_RATES.iter().any(|rate| rate * 100 == *current));
        let rates = CPU_RATES.iter().map(|rate| rate * 100).chain(other);
        for rate in rates {
            let factor = if rate % 100 == 0 {
                json!(rate / 100)
            } else {
                json!(f64::from(rate) / 100.0)
            };
            let shown = factor.to_string();
            items.push(self.marked(
                Label::Text(rb_i18n::t_with(
                    "emulation.extras.cpuRate",
                    &[("rate", &shown)],
                )),
                Mark::Radio(emulation.cpu == Some(rate)),
                CPU,
                json!({ "target": view, "rate": factor }),
            ));
        }
        self.submenu(Label::Key("emulation.extras.cpu"), items)
    }

    /// The text direction for `view`: the page's own or one forced (`dir: null` drops it).
    fn direction(&mut self, dir: Option<TextDirection>, view: &Value) -> Item {
        let choices = [
            ("contextmenu.emulation.dir_none", None, Value::Null),
            (
                "contextmenu.emulation.dir_ltr",
                Some(TextDirection::Ltr),
                json!("ltr"),
            ),
            (
                "contextmenu.emulation.dir_rtl",
                Some(TextDirection::Rtl),
                json!("rtl"),
            ),
            (
                "contextmenu.emulation.dir_auto",
                Some(TextDirection::Auto),
                json!("auto"),
            ),
        ];
        let items = choices
            .into_iter()
            .map(|(key, value, field)| {
                self.set(
                    key,
                    Mark::Radio(dir == value),
                    view,
                    json!({ "dir": field }),
                )
            })
            .collect();
        self.submenu(Label::Key("contextmenu.emulation.dir"), items)
    }

    /// Right to left for `target`: checked when `on`, and the choice turns it the other way
    /// (back to the page's own direction).
    fn rtl(&mut self, on: bool, target: &Value) -> Item {
        let dir = if on { Value::Null } else { json!("rtl") };
        self.set(
            "contextmenu.emulation.rtl",
            Mark::Check(on),
            target,
            json!({ "dir": dir }),
        )
    }

    /// The pseudo-localisation for `target`: checked when `on`, the choice turns it the other
    /// way.
    fn pseudo(&mut self, on: bool, target: &Value) -> Item {
        self.marked(
            Label::Key("contextmenu.emulation.pseudo"),
            Mark::Check(on),
            PSEUDO_LOCALE,
            json!({ "target": target, "on": !on }),
        )
    }

    /// The Emulation submenu for the clicked view. "All views" is marked by what every view
    /// has, not by the clicked one: a mixed state is unmarked, and the choice then turns the
    /// switch on for all.
    pub(super) fn emulation(&mut self, emulation: &EmulationState) -> Item {
        let view = json!(self.state.view);
        let all = json!("all");
        let mut items = self.scheme(Some(emulation.color_scheme), &view);
        items.push(Item::Separator);
        items.push(self.motion(emulation.reduced_motion, &view));
        items.push(self.marked(
            Label::Key("emulation.extras.offline"),
            Mark::Check(emulation.offline),
            OFFLINE,
            json!({ "target": view, "offline": !emulation.offline }),
        ));
        items.push(Item::Separator);
        items.push(self.location(emulation, &view));
        items.push(self.timezone(emulation, &view));
        items.push(self.cpu(emulation, &view));
        items.push(self.clock_menu(&emulation.time));
        items.push(self.animations_menu(&emulation.time, &view));
        items.push(Item::Separator);
        items.push(self.direction(emulation.dir, &view));
        items.push(self.pseudo(emulation.pseudo, &view));
        items.push(Item::Separator);
        items.push(self.reset(&view));
        let mut for_all = self.scheme(emulation.all.color_scheme, &all);
        for_all.push(Item::Separator);
        for_all.push(self.motion(emulation.all.reduced_motion, &all));
        for_all.push(self.rtl(emulation.all.rtl, &all));
        for_all.push(self.pseudo(emulation.all.pseudo, &all));
        for_all.push(Item::Separator);
        for_all.push(self.reset(&all));
        items.push(self.submenu(Label::Key("contextmenu.emulation.all_views"), for_all));
        self.submenu(Label::Key(super::model::KEY_EMULATION), items)
    }
}

#[cfg(test)]
#[path = "emulation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "emulation_marks_tests.rs"]
mod marks_tests;

#[cfg(test)]
#[path = "emulation_cpu_tests.rs"]
mod cpu_tests;

#[cfg(test)]
#[path = "emulation_dir_tests.rs"]
mod dir_tests;

#[cfg(test)]
#[path = "emulation_time_tests.rs"]
mod time_tests;
