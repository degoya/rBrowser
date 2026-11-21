//! The [`ViewHost`] contract on top of the host's state, window and CDP session.

use async_trait::async_trait;
use rb_config::{Breakpoint, DevtoolsMode, EmulationFields, Layout};
use rb_engine::{Emulation, EngineError, Target, ViewEvent, ViewHost, ViewId, ViewInfo};
use serde_json::Value;
use tokio::sync::broadcast;

use super::Host;
use super::nav::normalize_url;
use super::state::{emulation_for, layout_with_extra_column};
use crate::emulation::device_list;
use crate::emulation::fields::{overlay, reload_required, view_emulation};
use crate::fit::scale_input;

#[async_trait]
impl ViewHost for Host {
    fn views(&self) -> Vec<ViewInfo> {
        let state = self.state();
        state
            .order
            .iter()
            .filter_map(|id| state.views.get(id))
            .map(|entry| state.info(entry))
            .collect()
    }

    fn view(&self, view: ViewId) -> Option<ViewInfo> {
        let state = self.state();
        state.views.get(&view).map(|entry| state.info(entry))
    }

    fn active_view(&self) -> Option<ViewId> {
        self.state().active
    }

    fn set_active_view(&self, view: ViewId) -> Result<(), EngineError> {
        // Focused even when it is active already: the palette had the focus (RB-060-11).
        self.activate(view, true)
    }

    fn set_breakpoint(&self, view: ViewId, breakpoint: &Breakpoint) -> Result<(), EngineError> {
        {
            let mut state = self.state();
            let defaults = state.config.emulation.clone();
            let entry = state
                .views
                .get_mut(&view)
                .ok_or(EngineError::UnknownView(view))?;
            // The live overrides (`emulation.set`) stay over the new breakpoint.
            entry.emulation = view_emulation(
                Some(breakpoint),
                &entry.emulation,
                &defaults,
                &entry.overrides,
                &device_list(),
            )
            .unwrap_or_else(|_| emulation_for(breakpoint, &defaults));
            entry.breakpoint = Some(breakpoint.clone());
        }
        self.spawn_apply(view, true);
        self.emit(ViewEvent::Updated(view));
        Ok(())
    }

    fn set_size(
        &self,
        view: ViewId,
        width: u32,
        height: u32,
        dpr: Option<f64>,
    ) -> Result<(), EngineError> {
        if width == 0 || height == 0 || dpr.is_some_and(|dpr| !(dpr > 0.0 && dpr <= 10.0)) {
            return Err(EngineError::Invalid(format!(
                "{width}x{height} at DPR {dpr:?} is not a size"
            )));
        }
        {
            let mut state = self.state();
            let entry = state
                .views
                .get_mut(&view)
                .ok_or(EngineError::UnknownView(view))?;
            entry.emulation.width = width;
            entry.emulation.height = height;
            if let Some(dpr) = dpr {
                entry.emulation.dpr = dpr;
            }
            entry.breakpoint = None;
        }
        self.spawn_apply(view, true);
        self.emit(ViewEvent::Updated(view));
        Ok(())
    }

    fn apply_emulation(&self, view: ViewId, emulation: &Emulation) -> Result<(), EngineError> {
        self.state()
            .views
            .get_mut(&view)
            .ok_or(EngineError::UnknownView(view))?
            .emulation = emulation.clone();
        self.spawn_apply(view, true);
        self.emit(ViewEvent::Updated(view));
        Ok(())
    }

    fn emulation(&self, view: ViewId) -> Option<Emulation> {
        self.state()
            .views
            .get(&view)
            .map(|entry| entry.emulation.clone())
    }

    fn open_view(&self, breakpoint: &Breakpoint) -> Result<ViewId, EngineError> {
        let layout = {
            let mut state = self.state();
            if state.breakpoint(&breakpoint.id).is_none() {
                state.extra_breakpoints.push(breakpoint.clone());
            }
            layout_with_extra_column(&state.layout, &breakpoint.id)
        };
        let order = self.rebuild(&layout)?;
        order
            .last()
            .copied()
            .ok_or_else(|| EngineError::Invalid("the view was not opened".to_owned()))
    }

    fn set_layout(&self, layout: &Layout) -> Result<(), EngineError> {
        self.rebuild(layout).map(|_| ())
    }

    fn navigate(&self, target: Target, url: &str) -> Result<(), EngineError> {
        let url = normalize_url(url)?;
        for view in self.targets(target)? {
            let waiting = {
                let mut state = self.state();
                match state.views.get_mut(&view) {
                    Some(entry) if entry.session.is_none() => {
                        entry.pending_url = Some(url.clone());
                        true
                    }
                    _ => false,
                }
            };
            if !waiting {
                self.ui.navigate(view, &url);
            }
        }
        Ok(())
    }

    fn reload(&self, target: Target, ignore_cache: bool) -> Result<(), EngineError> {
        for view in self.targets(target)? {
            self.ui.reload(view, ignore_cache);
        }
        Ok(())
    }

    fn go_back(&self, target: Target) -> Result<(), EngineError> {
        for view in self.targets(target)? {
            self.ui.go_back(view);
        }
        Ok(())
    }

    fn go_forward(&self, target: Target) -> Result<(), EngineError> {
        for view in self.targets(target)? {
            self.ui.go_forward(view);
        }
        Ok(())
    }

    fn open_devtools(&self, view: ViewId, mode: DevtoolsMode) -> Result<(), EngineError> {
        if !self.state().views.contains_key(&view) {
            return Err(EngineError::UnknownView(view));
        }
        // RB-010-06's (contract 0.1-w1b).
        crate::devtools::open_devtools(self, view, mode)
    }

    async fn cdp_send(
        &self,
        view: ViewId,
        method: &str,
        params: Value,
    ) -> Result<Value, EngineError> {
        let (session, scale) = {
            let state = self.state();
            let entry = state
                .views
                .get(&view)
                .ok_or(EngineError::UnknownView(view))?;
            let session = entry.session.clone().ok_or_else(|| {
                EngineError::Invalid(format!("view {view} has no CDP session yet"))
            })?;
            let scale = entry
                .applied
                .map_or_else(|| state.metrics_of(entry).scale, |metrics| metrics.scale);
            (session, scale)
        };
        let mut params = params;
        scale_input(method, &mut params, scale);
        self.cdp.call(method, params, Some(&session)).await
    }

    fn subscribe(&self) -> broadcast::Receiver<ViewEvent> {
        self.events.subscribe()
    }
}

/// Emulation per view (RB-010-05): the live overrides of `emulation.set` and resolving again.
impl Host {
    /// The views of `target`; `None` is the active view (the first one when none is active).
    pub fn target_views(&self, target: Option<Target>) -> Result<Vec<ViewId>, EngineError> {
        match target {
            Some(target) => self.targets(target),
            None => self.active_or_first().map(|view| vec![view]),
        }
    }

    /// Puts `fields` over `view`'s live overrides and applies the result without a reload;
    /// `true` when the page shows part of it only after one ([`reload_required`]).
    pub fn set_emulation_fields(
        &self,
        view: ViewId,
        fields: &EmulationFields,
    ) -> Result<bool, EngineError> {
        self.re_emulate(view, |overrides| *overrides = overlay(overrides, fields))
    }

    /// Drops `view`'s live colour scheme override: back to its breakpoint's.
    pub fn clear_color_scheme(&self, view: ViewId) -> Result<bool, EngineError> {
        self.re_emulate(view, |overrides| overrides.color_scheme = None)
    }

    /// Drops `view`'s live text direction (`dir`) and/or language (`lang`): back to its
    /// breakpoint's, else the page's own (RB-092-02).
    pub fn clear_text_direction(
        &self,
        view: ViewId,
        dir: bool,
        lang: bool,
    ) -> Result<bool, EngineError> {
        self.re_emulate(view, |overrides| {
            if dir {
                overrides.dir = None;
            }
            if lang {
                overrides.lang = None;
            }
        })
    }

    /// Drops `view`'s live overrides: back to its breakpoint.
    pub fn reset_emulation(&self, view: ViewId) -> Result<bool, EngineError> {
        self.re_emulate(view, |overrides| *overrides = EmulationFields::default())
    }

    /// Resolves every view's emulation again (the config, its local headers or the device
    /// presets changed) and applies those that changed. A view that does not resolve keeps its
    /// emulation, with a warning.
    pub fn refresh_emulation(&self) {
        let devices = device_list();
        let changed: Vec<ViewId> = {
            let mut state = self.state();
            let defaults = state.config.emulation.clone();
            state
                .views
                .values_mut()
                .filter_map(|entry| {
                    let next = view_emulation(
                        entry.breakpoint.as_ref(),
                        &entry.emulation,
                        &defaults,
                        &entry.overrides,
                        &devices,
                    );
                    match next {
                        Ok(next) if next != entry.emulation => {
                            entry.emulation = next;
                            Some(entry.id)
                        }
                        Ok(_) => None,
                        Err(error) => {
                            tracing::warn!(view = %entry.id, %error, "the emulation was not resolved again");
                            None
                        }
                    }
                })
                .collect()
        };
        for view in changed {
            self.spawn_apply(view, false);
            self.emit(ViewEvent::Updated(view));
        }
    }

    /// Changes `view`'s overrides with `change`, resolves and applies; nothing changes when the
    /// result does not resolve.
    fn re_emulate(
        &self,
        view: ViewId,
        change: impl FnOnce(&mut EmulationFields),
    ) -> Result<bool, EngineError> {
        let devices = device_list();
        let reload = {
            let mut state = self.state();
            let defaults = state.config.emulation.clone();
            let entry = state
                .views
                .get_mut(&view)
                .ok_or(EngineError::UnknownView(view))?;
            let mut overrides = entry.overrides.clone();
            change(&mut overrides);
            let next = view_emulation(
                entry.breakpoint.as_ref(),
                &entry.emulation,
                &defaults,
                &overrides,
                &devices,
            )?;
            let reload = reload_required(&entry.emulation, &next);
            entry.overrides = overrides;
            entry.emulation = next;
            reload
        };
        self.spawn_apply(view, false);
        self.emit(ViewEvent::Updated(view));
        Ok(reload)
    }
}
