pub mod style;
pub mod components;

use macroquad::prelude::*;
use style::*;
use components::*;

pub fn draw_editor() {
    let sw = screen_width();
    let sh = screen_height();
    let layout = compute_layout(sw, sh);

    clear_background(BG_DARK);

    toolbar::Toolbar::draw(layout.toolbar);
    sidebar::Sidebar::draw(layout.sidebar);
    viewport::Viewport::draw(layout.viewport);
    panel::RightPanel::draw(layout.right_panel);
    timeline::Timeline::draw(layout.timeline);
}
