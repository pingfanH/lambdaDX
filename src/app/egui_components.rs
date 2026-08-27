use super::egui_style::*;
use egui_macroquad::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Stroke, StrokeKind, Vec2,
};

// ── Buttons ──

pub struct Button<'a> {
    pub label: &'a str,
    pub kind: ButtonKind,
    pub size: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Normal,
    Primary,
    Toggle(bool),
    Icon,
    Separator,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str, kind: ButtonKind, size: Vec2) -> Self {
        Self { label, kind, size }
    }

    pub fn show(self, ui: &mut egui::Ui) -> bool {
        if self.kind == ButtonKind::Separator {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(1.0, self.size.y), egui::Sense::hover());
            ui.painter().line_segment(
                [
                    egui::pos2(rect.center().x, rect.top() + 4.0),
                    egui::pos2(rect.center().x, rect.bottom() - 4.0),
                ],
                Stroke::new(1.0, SEPARATOR),
            );
            return false;
        }

        let bg = match self.kind {
            ButtonKind::Primary => ACCENT_BLUE,
            ButtonKind::Toggle(on) if on => BG_BUTTON_HOVER,
            _ => BG_BUTTON,
        };

        let text_color = match self.kind {
            ButtonKind::Primary => Color32::WHITE,
            ButtonKind::Toggle(false) => TEXT_DIM,
            _ => TEXT_PRIMARY,
        };

        let btn = ui.allocate_response(self.size, egui::Sense::click());
        let rect = btn.rect;

        ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, BUTTON_BORDER),
            StrokeKind::Outside,
        );

        if btn.hovered() {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(4),
                Color32::from_rgba_premultiplied(255, 255, 255, 10),
            );
        }

        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            self.label,
            FontId::proportional(FONT_BUTTON),
            text_color,
        );

        btn.clicked()
    }
}

pub fn icon_button(ui: &mut egui::Ui, icon: &str, active: bool) -> bool {
    let size = Vec2::splat(ICON_SIZE);
    let bg = if active { ACCENT_BLUE } else { BG_BUTTON };

    let btn = ui.allocate_response(size, egui::Sense::click());
    let rect = btn.rect;

    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0, BUTTON_BORDER),
        StrokeKind::Outside,
    );

    if btn.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(5),
            Color32::from_rgba_premultiplied(255, 255, 255, 10),
        );
    }

    let text_color = if active {
        Color32::WHITE
    } else {
        TEXT_SECONDARY
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(FONT_ICON),
        text_color,
    );

    btn.clicked()
}

pub fn section_header(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(TEXT_PRIMARY)
                .size(FONT_HEADER),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("▼").color(TEXT_DIM).size(FONT_SMALL));
        });
    });
}

pub fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(TEXT_SECONDARY)
                .size(FONT_BODY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(value)
                    .color(TEXT_PRIMARY)
                    .size(FONT_BODY),
            );
        });
    });
}

pub fn separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, BORDER_LIGHT);
}

pub fn hsep(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    separator(ui);
    ui.add_space(2.0);
}

pub fn toggle_btn(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(if active {
            Color32::WHITE
        } else {
            TEXT_PRIMARY
        }))
        .fill(if active { BG_ACTIVE } else { BG_BUTTON }),
    )
    .clicked()
}

pub fn step_btn(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(TEXT_PRIMARY))
            .min_size(Vec2::new(20.0, 20.0)),
    )
}

pub fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(TEXT_DIM).small());
}

// ── Sidebar ──

pub struct SidebarTool {
    pub icon: &'static str,
    pub tooltip: &'static str,
    pub active: bool,
}

pub fn draw_sidebar(ui: &mut egui::Ui, tools: &[SidebarTool], on_click: &mut dyn FnMut(usize)) {
    egui::Frame::new()
        .fill(BG_SIDEBAR)
        .stroke(Stroke::new(1.0, BORDER_LIGHT))
        .inner_margin(egui::Margin::same(PADDING as i8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING;

                for (i, tool) in tools.iter().enumerate() {
                    if icon_button(ui, tool.icon, tool.active) {
                        on_click(i);
                    }
                }
            });
        });
}

// ── Sliding Drawers ──

static mut DRAWER_OPEN: bool = false;
static mut DRAWER_ANIM: f32 = 0.0;
static mut DRAWER_LAST_TIME: f64 = -1.0;

const ANIM_SPEED: f32 = 5.0;

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Content callback for the templates drawer.
pub type DrawerContentFn = Box<dyn FnMut(&mut egui::Ui, f32)>;

pub fn draw_drawer(
    ctx: &egui::Context,
    screen_rect: egui::Rect,
    id: &str,
    title: &str,
    content: &mut [(&str, usize, bool)],
    on_select: &mut dyn FnMut(usize),
    on_new: &mut dyn FnMut(),
) {
    // tick animation (inline to avoid unsafe refs in Rust 2024)
    unsafe {
        let now = ctx.input(|i| i.time);
        let last = DRAWER_LAST_TIME;
        let dt = if last < 0.0 {
            0.016
        } else {
            (now - last).min(0.1) as f32
        };
        DRAWER_LAST_TIME = now;

        let target: f32 = if DRAWER_OPEN { 1.0 } else { 0.0 };
        let cur = DRAWER_ANIM;
        let new_anim = if (cur - target).abs() < 0.005 {
            target
        } else {
            let step = (target - cur) * (ANIM_SPEED * dt).min(1.0);
            let min_step = dt * 3.0;
            if step.abs() < min_step {
                cur + min_step.copysign(step)
            } else {
                cur + step
            }
            .clamp(0.0, 1.0)
        };
        DRAWER_ANIM = new_anim;

        if (new_anim - target).abs() > 0.001 {
            ctx.request_repaint();
        }
    }

    let (open, anim) = unsafe { (DRAWER_OPEN, DRAWER_ANIM) };
    if anim < 0.005 {
        return;
    }

    let drawer_w = RIGHT_PANEL_WIDTH + PADDING * 2.0;
    let eased = if open {
        ease_out(anim)
    } else {
        1.0 - ease_out(1.0 - anim)
    };

    let x_offset = drawer_w * (1.0 - eased);
    let drawer_x = screen_rect.right() - drawer_w + x_offset;
    let drawer_rect = egui::Rect::from_min_size(
        Pos2::new(drawer_x, screen_rect.top()),
        Vec2::new(drawer_w, screen_rect.height()),
    );

    let bg_alpha = (eased * 200.0).min(200.0) as u8;

    let btn_w = BUTTON_HEIGHT * 0.6;
    let btn_h = ICON_SIZE * 2.0;
    let btn_rect = egui::Rect::from_center_size(
        Pos2::new(drawer_rect.left(), drawer_rect.center().y),
        Vec2::new(btn_w, btn_h),
    );
    let hit_rect = if open {
        drawer_rect.union(btn_rect)
    } else {
        drawer_rect
    };

    egui::Area::new(egui::Id::new(id))
        .fixed_pos(drawer_rect.left_top())
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                drawer_rect,
                CornerRadius::ZERO,
                Color32::from_rgba_premultiplied(42, 42, 46, bg_alpha),
            );
            ui.painter().rect_stroke(
                drawer_rect,
                CornerRadius::ZERO,
                Stroke::new(1.0, BORDER_LIGHT),
                StrokeKind::Outside,
            );

            if open {
                let resp = ui.allocate_rect(btn_rect, egui::Sense::click());
                let bg = if resp.hovered() {
                    BG_BUTTON_HOVER
                } else {
                    BG_BUTTON
                };
                ui.painter()
                    .rect_filled(btn_rect, CornerRadius::same(4), bg);
                ui.painter().rect_stroke(
                    btn_rect,
                    CornerRadius::same(4),
                    Stroke::new(1.0, BUTTON_BORDER),
                    StrokeKind::Outside,
                );
                ui.painter().text(
                    btn_rect.center(),
                    Align2::CENTER_CENTER,
                    "▶",
                    FontId::proportional(FONT_BODY),
                    TEXT_PRIMARY,
                );
                if resp.clicked() {
                    unsafe {
                        DRAWER_OPEN = false;
                    }
                }
            }

            let input = ui.input(|i| (i.pointer.primary_clicked(), i.pointer.latest_pos()));
            if let (true, Some(pos)) = input {
                if !hit_rect.contains(pos) {
                    unsafe {
                        DRAWER_OPEN = false;
                    }
                }
            }

            let content_rect = egui::Rect::from_min_size(
                Pos2::new(
                    drawer_rect.left() + btn_w * 0.5 + PADDING,
                    drawer_rect.top(),
                ),
                Vec2::new(
                    drawer_rect.width() - btn_w * 0.5 - PADDING * 2.0,
                    drawer_rect.height(),
                ),
            );

            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content_rect.shrink(PADDING))
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );

            child_ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING;
                section_header(ui, title);
                ui.add_space(SPACING * 1.5);

                for (idx, (name, count, selected)) in content.iter().enumerate() {
                    let bg = if *selected {
                        Color32::from_rgb(46, 71, 115)
                    } else {
                        BG_PANEL
                    };
                    let item_h = BUTTON_HEIGHT * 1.3;

                    let btn = ui.allocate_response(
                        Vec2::new(ui.available_width(), item_h),
                        egui::Sense::click(),
                    );
                    ui.painter()
                        .rect_filled(btn.rect, CornerRadius::same(6), bg);
                    ui.painter().rect_stroke(
                        btn.rect,
                        CornerRadius::same(6),
                        Stroke::new(1.0, BORDER_LIGHT),
                        StrokeKind::Outside,
                    );

                    ui.painter().text(
                        egui::pos2(btn.rect.left() + PADDING, btn.rect.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(FONT_BODY),
                        TEXT_PRIMARY,
                    );

                    if *count > 0 {
                        let count_text = format!("×{}", count);
                        ui.painter().text(
                            egui::pos2(btn.rect.right() - PADDING, btn.rect.center().y),
                            Align2::RIGHT_CENTER,
                            &count_text,
                            FontId::proportional(FONT_BODY),
                            TEXT_DIM,
                        );
                    }
                    if btn.clicked() {
                        on_select(idx);
                    }
                }

                ui.add_space(SPACING);
                if Button::new(
                    "+ New",
                    ButtonKind::Normal,
                    Vec2::new(ui.available_width(), BUTTON_HEIGHT),
                )
                .show(ui)
                {
                    on_new();
                }
            });
        });
}

/// Toggle button shown on right edge when drawer is closed.
pub fn draw_drawer_toggle(ui: &mut egui::Ui, viewport_rect: egui::Rect) {
    let anim = unsafe { DRAWER_ANIM };
    if anim > 0.05 {
        return;
    }

    let btn_w = BUTTON_HEIGHT * 0.6;
    let btn_h = ICON_SIZE * 2.0;
    let btn_rect = egui::Rect::from_center_size(
        Pos2::new(
            viewport_rect.right() - btn_w * 0.5,
            viewport_rect.center().y,
        ),
        Vec2::new(btn_w, btn_h),
    );

    let resp = ui.allocate_rect(btn_rect, egui::Sense::click());
    let bg = if resp.hovered() {
        BG_BUTTON_HOVER
    } else {
        BG_BUTTON
    };
    ui.painter()
        .rect_filled(btn_rect, CornerRadius::same(4), bg);
    ui.painter().rect_stroke(
        btn_rect,
        CornerRadius::same(4),
        Stroke::new(1.0, BUTTON_BORDER),
        StrokeKind::Outside,
    );
    ui.painter().text(
        btn_rect.center(),
        Align2::CENTER_CENTER,
        "◀",
        FontId::proportional(FONT_BODY),
        TEXT_PRIMARY,
    );

    if resp.clicked() {
        unsafe {
            DRAWER_OPEN = true;
        }
    }
}
