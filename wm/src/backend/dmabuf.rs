//! Linux-dmabuf advertisement and asynchronous import validation. Import is
//! deferred to the backend's next frame so a paused session can keep clients
//! waiting until its renderer becomes available again.

use smithay::{
    backend::{allocator::{Format, dmabuf::Dmabuf}, drm::DrmNode},
    reexports::{calloop::timer::{Timer, TimeoutAction}, wayland_server::DisplayHandle},
    wayland::dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
};
use std::time::Duration;
use crate::state::State;

pub struct DmabufSupport {
    pub state: DmabufState,
    /// The GPU's primary dev_t; feedback may name its render node instead.
    pub device: Option<u64>,
    global: Option<DmabufGlobal>,
    node: Option<DrmNode>,
    pending: Vec<(Dmabuf, ImportNotifier)>,
}

impl Default for DmabufSupport {
    fn default() -> Self {
        Self { state: DmabufState::new(), device: None, global: None, node: None, pending: Vec::new() }
    }
}

impl DmabufSupport {
    pub fn enable(&mut self, display: &DisplayHandle, device: u64, node: Option<DrmNode>, formats: impl IntoIterator<Item = Format>) -> Result<Option<DmabufGlobal>, Box<dyn std::error::Error>> {
        if self.global.is_some() { return Ok(self.global); }
        let formats: Vec<_> = formats.into_iter().collect();
        if formats.is_empty() { return Ok(None); }
        self.global = Some(if let Some(node) = node {
            let feedback = DmabufFeedbackBuilder::new(node.dev_id(), formats).build()?;
            self.state.create_global_with_default_feedback::<State>(display, &feedback)
        } else {
            self.state.create_global::<State>(display, formats)
        });
        self.device = Some(device);
        self.node = node;
        Ok(self.global)
    }

    pub fn disable(&mut self, display: &DisplayHandle) -> Option<DmabufGlobal> {
        self.device = None;
        self.node = None;
        let global = self.global.take()?;
        self.state.disable_global::<State>(display, &global);
        self.process_imports(|_| false);
        Some(global)
    }

    pub fn process_imports(&mut self, mut import: impl FnMut(&Dmabuf) -> bool) {
        for (dmabuf, notifier) in self.pending.drain(..) {
            if import(&dmabuf) {
                if dmabuf.node().is_none() && let Some(node) = self.node { dmabuf.set_node(node); }
                tracing::trace!("dmabuf import accepted");
                let _ = notifier.successful::<State>();
            } else {
                tracing::debug!("dmabuf import rejected");
                notifier.failed();
            }
        }
    }
}

impl State {
    pub fn disable_dmabuf(&mut self) {
        let Some(global) = self.dmabuf.disable(&self.display_handle) else { return };
        // Retain the disabled global briefly for clients racing registry removal.
        if let Err(err) = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(10)), move |_, _, state| {
            state.dmabuf.state.destroy_global::<State>(&state.display_handle, global);
            TimeoutAction::Drop
        }) { tracing::warn!(?err, "dmabuf global cleanup timer failed"); }
    }
}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState { &mut self.dmabuf.state }

    fn dmabuf_imported(&mut self, global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        if self.dmabuf.global.as_ref() == Some(global) {
            self.dmabuf.pending.push((dmabuf, notifier));
        } else {
            tracing::debug!("dmabuf import rejected: GPU removed");
            notifier.failed();
        }
    }
}
