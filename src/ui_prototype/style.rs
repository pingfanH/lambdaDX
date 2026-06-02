use macroquad::prelude::Color;

// ── Colors (from Bevy Editor SVG) ──
pub const BG_DARK: Color = Color::new(0.122, 0.122, 0.141, 1.0);      // #1F1F24
pub const BG_PANEL: Color = Color::new(0.165, 0.165, 0.180, 1.0);     // #2A2A2E
pub const BG_BUTTON: Color = Color::new(0.212, 0.216, 0.231, 1.0);    // #36373B
pub const BG_BUTTON_HOVER: Color = Color::new(0.275, 0.278, 0.298, 1.0);
pub const BG_INPUT: Color = Color::new(0.275, 0.278, 0.298, 1.0);     // #46474C
pub const ACCENT_BLUE: Color = Color::new(0.125, 0.427, 0.788, 1.0);  // #206EC9
pub const ACCENT_YELLOW: Color = Color::new(1.0, 0.792, 0.224, 1.0);  // #FFCA39
pub const TEXT_PRIMARY: Color = Color::new(0.925, 0.925, 0.925, 1.0);  // #ECECEC
pub const TEXT_SECONDARY: Color = Color::new(0.659, 0.659, 0.659, 1.0); // #A8A8A8
pub const TEXT_DIM: Color = Color::new(0.514, 0.514, 0.522, 1.0);     // #838385
pub const BORDER: Color = Color::new(0.251, 0.251, 0.251, 1.0);       // #404040
pub const BORDER_LIGHT: Color = Color::new(0.188, 0.188, 0.188, 1.0); // #303030
pub const BUTTON_BORDER: Color = Color::new(0.255, 0.255, 0.259, 1.0); // #414142
pub const SEPARATOR: Color = Color::new(1.0, 1.0, 1.0, 0.06);

// ── Sizes ──
pub const TOOLBAR_HEIGHT: f32 = 30.0;
pub const SIDEBAR_WIDTH: f32 = 52.0;
pub const RIGHT_PANEL_WIDTH: f32 = 280.0;
pub const TIMELINE_HEIGHT: f32 = 180.0;
pub const BUTTON_HEIGHT: f32 = 26.0;
pub const BUTTON_RADIUS: f32 = 4.0;
pub const ICON_SIZE: f32 = 18.0;
pub const SPACING: f32 = 6.0;
pub const PADDING: f32 = 8.0;

// ── Layout ──
pub struct Layout {
    pub toolbar: UIRect,
    pub sidebar: UIRect,
    pub viewport: UIRect,
    pub right_panel: UIRect,
    pub timeline: UIRect,
}

#[derive(Debug, Clone, Copy)]
pub struct UIRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl UIRect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }

    pub fn inset(&self, padding: f32) -> Self {
        Self {
            x: self.x + padding,
            y: self.y + padding,
            w: self.w - padding * 2.0,
            h: self.h - padding * 2.0,
        }
    }
}

pub fn compute_layout(screen_w: f32, screen_h: f32) -> Layout {
    let margin = 10.0;
    let inner_x = margin;
    let inner_y = margin;
    let inner_w = screen_w - margin * 2.0;
    let inner_h = screen_h - margin * 2.0;

    let toolbar = UIRect::new(inner_x, inner_y, inner_w, TOOLBAR_HEIGHT);
    let sidebar = UIRect::new(inner_x, toolbar.y + toolbar.h + SPACING, SIDEBAR_WIDTH, inner_h - TOOLBAR_HEIGHT - TIMELINE_HEIGHT - SPACING * 2.0);
    let right_panel = UIRect::new(inner_x + inner_w - RIGHT_PANEL_WIDTH, sidebar.y, RIGHT_PANEL_WIDTH, sidebar.h);
    let viewport = UIRect::new(sidebar.x + sidebar.w + SPACING, sidebar.y, right_panel.x - sidebar.x - sidebar.w - SPACING * 2.0, sidebar.h);
    let timeline = UIRect::new(inner_x, sidebar.y + sidebar.h + SPACING, inner_w, TIMELINE_HEIGHT);

    Layout { toolbar, sidebar, viewport, right_panel, timeline }
}
