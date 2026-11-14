//! The clipboard and the primary selection (select, then middle-click),
//! each seat's own: what one seat copies, another doesn't paste. Clipboard
//! tools (`wl-copy`, `wl-paste --watch cliphist store`) reach a seat's
//! clipboard and selection with no window of their own, through data
//! control (`wlr-data-control` and `ext-data-control`); `wl-paste` takes the
//! first seat unless given `--seat`.

use smithay::{
    reexports::wayland_server::DisplayHandle,
    wayland::selection::{
        ext_data_control::{self, DataControlHandler as ExtDataControlHandler},
        primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
        wlr_data_control::{self, DataControlHandler as WlrDataControlHandler},
    },
};

use crate::state::State;

pub struct Clipboard {
    pub primary: PrimarySelectionState,
    pub wlr: wlr_data_control::DataControlState,
    pub ext: ext_data_control::DataControlState,
}

impl Clipboard {
    pub fn new(dh: &DisplayHandle) -> Self {
        let primary = PrimarySelectionState::new::<State>(dh);
        let wlr = wlr_data_control::DataControlState::new::<State, _>(dh, Some(&primary), |_| true);
        let ext = ext_data_control::DataControlState::new::<State, _>(dh, Some(&primary), |_| true);
        Self { primary, wlr, ext }
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.clipboard.primary
    }
}

impl WlrDataControlHandler for State {
    fn data_control_state(&mut self) -> &mut wlr_data_control::DataControlState {
        &mut self.clipboard.wlr
    }
}

impl ExtDataControlHandler for State {
    fn data_control_state(&mut self) -> &mut ext_data_control::DataControlState {
        &mut self.clipboard.ext
    }
}
