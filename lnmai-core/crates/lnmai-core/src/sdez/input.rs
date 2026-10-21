//! SDEZ170 touch-panel input log.

pub const PANEL_AREA_COUNT: usize = 35;

pub const A1: usize = 0;
pub const A2: usize = 1;
pub const A3: usize = 2;
pub const A8: usize = 7;
pub const B1: usize = 8;
pub const B8: usize = 15;
pub const C1: usize = 16;
pub const C2: usize = 17;
pub const D1: usize = 18;
pub const D8: usize = 25;
pub const E1: usize = 26;
pub const E8: usize = 33;

/// Three-generation panel state and the current hardware-frame marker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelLog {
    now: u64,
    last: u64,
    old: u64,
    trigger_frame: bool,
}

impl PanelLog {
    /// GameInputLog.Execute. new_frame is false when the hardware counter did
    /// not change; in that case no generation is shifted and TriggerOn is off.
    pub fn update(&mut self, now: u64, new_frame: bool) {
        self.trigger_frame = false;
        if !new_frame {
            return;
        }
        self.old = self.last;
        self.last = self.now;
        self.now = now;
        self.trigger_frame = true;
        for area in 0..PANEL_AREA_COUNT {
            self.kill_on_off_on(area);
        }
    }

    #[inline]
    pub fn state_on(&self, area: usize) -> bool {
        bit(self.now, area)
    }

    #[inline]
    pub fn trigger_on(&self, area: usize) -> bool {
        self.trigger_frame && !bit(self.last, area) && bit(self.now, area)
    }

    #[inline]
    pub fn last_on(&self, area: usize) -> bool {
        bit(self.last, area)
    }

    #[inline]
    pub fn old_on(&self, area: usize) -> bool {
        bit(self.old, area)
    }

    fn kill_on_off_on(&mut self, area: usize) {
        if self.old_on(area) && !self.last_on(area) && self.state_on(area) {
            self.now &= !(1u64 << area);
        }
    }
}

/// Per-frame input surface: panel history, this-frame ownership, and the
/// button down/push pair. Button indices are 0..8, matching ButtonSetting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputFrame {
    pub panel: PanelLog,
    panel_used: u64,
    buttons_down: u16,
    buttons_push: u16,
}

impl InputFrame {
    pub fn update_panel(&mut self, now: u64, new_frame: bool) {
        self.panel.update(now, new_frame);
    }

    pub fn set_buttons(&mut self, down: u16, push: u16) {
        self.buttons_down = down;
        self.buttons_push = push;
    }

    #[inline]
    pub fn panel_state(&self, area: usize) -> bool {
        self.panel.state_on(area)
    }

    #[inline]
    pub fn panel_trigger(&self, area: usize) -> bool {
        self.panel.trigger_on(area)
    }

    #[inline]
    pub fn panel_used(&self, area: usize) -> bool {
        bit(self.panel_used, area)
    }

    pub fn set_panel_used(&mut self, area: usize) {
        self.panel_used |= 1u64 << area;
    }

    pub fn clear_used(&mut self) {
        self.panel_used = 0;
    }

    #[inline]
    pub fn button_down(&self, button: usize) -> bool {
        bit(self.buttons_down as u64, button)
    }

    #[inline]
    pub fn button_push(&self, button: usize) -> bool {
        bit(self.buttons_push as u64, button)
    }

    /// The C center note owns both C1 and C2 when it judges.
    pub fn set_center_used(&mut self) {
        self.set_panel_used(C1);
        self.set_panel_used(C2);
    }
}

#[inline]
fn bit(bits: u64, index: usize) -> bool {
    index < 64 && (bits >> index) & 1 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_on_only_fires_on_a_new_frame() {
        let mut log = PanelLog::default();
        log.update(1 << A1, true);
        assert!(log.trigger_on(A1));
        log.update(1 << A1, false);
        assert!(!log.trigger_on(A1), "stale hardware frame cannot retrigger");
        assert!(log.state_on(A1));
    }

    #[test]
    fn state_trigger_uses_now_and_last() {
        let mut log = PanelLog::default();
        log.update(1 << A2, true);
        assert!(log.trigger_on(A2));
        log.update(0, true);
        assert!(!log.trigger_on(A2));
        assert!(!log.state_on(A2));
        assert!(log.last_on(A2));
    }

    #[test]
    fn on_off_on_is_suppressed() {
        let mut log = PanelLog::default();
        log.update(1 << B1, true);
        log.update(0, true);
        log.update(1 << B1, true);
        assert!(log.old_on(B1));
        assert!(!log.last_on(B1));
        assert!(!log.state_on(B1), "on-off-on glitch is cleared");
        assert!(!log.trigger_on(B1));
    }

    #[test]
    fn panel_ownership_and_center_pair() {
        let mut input = InputFrame::default();
        input.update_panel(1 << C1, true);
        assert!(input.panel_trigger(C1));
        assert!(!input.panel_trigger(C2));
        input.set_center_used();
        assert!(input.panel_used(C1));
        assert!(input.panel_used(C2));
        input.clear_used();
        assert!(!input.panel_used(C1));
    }

    #[test]
    fn button_down_and_push_are_distinct() {
        let mut input = InputFrame::default();
        input.set_buttons(1 << 0, 1 << 0);
        assert!(input.button_down(0));
        assert!(input.button_push(0));
        input.set_buttons(0, 1 << 0);
        assert!(!input.button_down(0));
        assert!(input.button_push(0));
    }
}
