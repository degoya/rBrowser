//! The Clock and Animations submenus of the page menu's Emulation (RB-094-07): the commands
//! `emulation.clock` (one clock for every view: the real time, the time stopped now, the
//! settings' clock; a running clock shown chosen) and `overlays.animations` for the clicked
//! view (the settings' rates, the view's own marked; Step while stopped) with 100 % for every
//! view. No logic of its own: the commands are `crate::time_control`'s. Tested with the
//! Emulation submenu (`emulation_time_tests.rs`).

use rb_config::EmulationUserSettings;
use rb_engine::ViewId;
use serde_json::{Value, json};

use super::model::{Action, Builder, Item, Label, Mark};
use crate::time_control::model::{is_normal, rates_of};
use crate::time_control::{ANIMATIONS, CLOCK};

/// One in ten-thousandths: rates compare exactly.
const ONE: u32 = 10_000;

/// What the two submenus show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeMenu {
    /// The views' clock while one runs: its start as the activity shows it, its speed in
    /// hundredths.
    pub clock: Option<(String, u32)>,
    /// `emulation.clock.default` and `emulation.clock.speed` (hundredths): the settings' clock,
    /// offered when it is more than the time stopped now.
    pub settings_clock: Option<(String, u32)>,
    /// The clicked view's rate in ten-thousandths.
    pub rate: u32,
    /// The rates offered, in ten-thousandths, in the settings' order.
    pub rates: Vec<u32>,
}

impl Default for TimeMenu {
    fn default() -> Self {
        Self::with(None, 1.0, &EmulationUserSettings::default())
    }
}

impl TimeMenu {
    /// The menu for the clicked `view` with the window's clock and rates and `settings`.
    #[must_use]
    pub fn of(view: ViewId, settings: &EmulationUserSettings) -> Self {
        let hooks = crate::time_control::hooks();
        let rate = hooks
            .rates()
            .into_iter()
            .find(|(known, _)| *known == view)
            .map_or(1.0, |(_, rate)| rate);
        let clock = hooks
            .clock()
            .map(|clock| (clock.start.text(), hundredths(clock.speed)));
        Self::with(clock, rate, settings)
    }

    /// The menu with `clock` running and the clicked view at `rate`.
    #[must_use]
    pub fn with(clock: Option<(String, u32)>, rate: f64, settings: &EmulationUserSettings) -> Self {
        let wanted = settings.clock.default.trim();
        let settings_clock = (!(wanted.is_empty() || wanted.eq_ignore_ascii_case("now"))
            || settings.clock.speed > 0.0)
            .then(|| (wanted.to_owned(), hundredths(settings.clock.speed)));
        Self {
            clock,
            settings_clock,
            rate: if is_normal(rate) {
                ONE
            } else {
                ten_thousandths(rate)
            },
            rates: rates_of(&settings.animation_rates)
                .into_iter()
                .map(ten_thousandths)
                .collect(),
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded and clamped to 0..=100 000 first: the cast neither wraps nor truncates"
)]
fn ten_thousandths(rate: f64) -> u32 {
    (rate * f64::from(ONE)).round().clamp(0.0, 100_000.0) as u32
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded and clamped to 0..=10 000 first: the cast neither wraps nor truncates"
)]
fn hundredths(speed: f64) -> u32 {
    (speed * 100.0).round().clamp(0.0, 10_000.0) as u32
}

/// `value` in hundredths as a number: `2`, `2.5`, `0.05`.
fn number(value: u32) -> Value {
    if value.is_multiple_of(100) {
        json!(value / 100)
    } else {
        json!(f64::from(value) / 100.0)
    }
}

/// A rate's name: Stopped, else its percentage.
fn rate_label(rate: u32) -> Label {
    if rate == 0 {
        Label::Key("contextmenu.time.stopped")
    } else {
        let percent = number(rate).to_string();
        Label::Text(rb_i18n::t_with(
            "contextmenu.time.rate",
            &[("percent", &percent)],
        ))
    }
}

impl Builder<'_> {
    fn time_entry(&mut self, label: Label, mark: Mark, command: &str, args: Value) -> Item {
        let action = Action::Run {
            command: command.to_owned(),
            args,
        };
        self.labelled(label, true, mark, action)
    }

    /// The clock of every view: the running one chosen (choosing it changes nothing), the real
    /// time, the time stopped now, the settings' clock.
    pub(super) fn clock_menu(&mut self, time: &TimeMenu) -> Item {
        let mut items = Vec::new();
        if let Some((start, speed)) = &time.clock {
            let speed_text = number(*speed).to_string();
            let label = Label::Text(if *speed == 0 {
                rb_i18n::t_with("contextmenu.time.clock_fixed", &[("start", start)])
            } else {
                rb_i18n::t_with(
                    "contextmenu.time.clock_running",
                    &[("start", start), ("speed", &speed_text)],
                )
            });
            let action = Action::Run {
                command: CLOCK.to_owned(),
                args: json!({ "speed": number(*speed) }),
            };
            items.push(self.labelled(label, false, Mark::Radio(true), action));
        }
        items.push(self.time_entry(
            Label::Key("contextmenu.time.real_time"),
            Mark::Radio(time.clock.is_none()),
            CLOCK,
            json!({ "reset": true }),
        ));
        items.push(self.time_entry(
            Label::Key("contextmenu.time.freeze_now"),
            Mark::Radio(false),
            CLOCK,
            json!({ "time": "now", "speed": 0 }),
        ));
        if let Some((start, speed)) = &time.settings_clock {
            let time_arg = if start.is_empty() { "now" } else { start };
            items.push(self.time_entry(
                Label::Key("contextmenu.time.settings_clock"),
                Mark::Radio(false),
                CLOCK,
                json!({ "time": time_arg, "speed": number(*speed) }),
            ));
        }
        self.submenu(Label::Key("contextmenu.time.clock"), items)
    }

    /// The animations of the clicked `view`: the settings' rates (a rate set elsewhere shown
    /// too), Step while stopped, and 100 % in every view.
    pub(super) fn animations_menu(&mut self, time: &TimeMenu, view: &Value) -> Item {
        let other = Some(time.rate).filter(|rate| !time.rates.contains(rate));
        let rates: Vec<u32> = time.rates.iter().copied().chain(other).collect();
        let mut items: Vec<Item> = rates
            .into_iter()
            .map(|rate| {
                self.time_entry(
                    rate_label(rate),
                    Mark::Radio(rate == time.rate),
                    ANIMATIONS,
                    json!({ "target": view, "rate": f64::from(rate) / f64::from(ONE) }),
                )
            })
            .collect();
        items.push(Item::Separator);
        let step = Action::Run {
            command: ANIMATIONS.to_owned(),
            args: json!({ "target": view, "step": true }),
        };
        items.push(self.labelled(
            Label::Key("contextmenu.time.step"),
            time.rate == 0,
            Mark::None,
            step,
        ));
        items.push(self.time_entry(
            Label::Key("contextmenu.time.all_normal"),
            Mark::None,
            ANIMATIONS,
            json!({ "target": "all", "rate": 1 }),
        ));
        self.submenu(Label::Key("contextmenu.time.animations"), items)
    }
}
