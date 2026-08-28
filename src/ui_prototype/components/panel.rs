use super::button::*;
use crate::ui_prototype::style::*;
use egui_macroquad::egui::{self, Color32, CornerRadius, Pos2, Stroke, StrokeKind, Vec2};

static mut DRAWER_OPEN: bool = false;
static mut ANIM: f32 = 0.0; // 0.0 = closed, 1.0 = open
static mut LAST_TIME: f64 = -1.0;

static mut SCENE_DRAWER_OPEN: bool = false;
static mut SCENE_ANIM: f32 = 0.0;
static mut SCENE_LAST_TIME: f64 = -1.0;

const ANIM_SPEED: f32 = 5.0; // higher = faster

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Draw the arrow toggle button on the right edge (only when drawer is closed).
pub fn draw_toggle_button(ui: &mut egui::Ui, viewport_rect: egui::Rect) {
    let anim = unsafe { ANIM };
    if anim > 0.05 {
        return; // hidden when drawer is opening/open
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
        Stroke::new(1.0_f32, BUTTON_BORDER),
        StrokeKind::Outside,
    );

    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "◀",
        egui::FontId::proportional(FONT_BODY),
        TEXT_PRIMARY,
    );

    if resp.clicked() {
        unsafe {
            DRAWER_OPEN = true;
        }
    }
}

/// Update animation progress. Call once per frame.
fn tick_animation(ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    let last = unsafe { LAST_TIME };
    let dt = if last < 0.0 {
        0.016
    } else {
        (now - last).min(0.1) as f32
    };
    unsafe {
        LAST_TIME = now;
    }

    let target = if unsafe { DRAWER_OPEN } { 1.0 } else { 0.0 };
    let anim = unsafe { ANIM };
    let new_anim = if (anim - target).abs() < 0.005 {
        target
    } else {
        let step = (target - anim) * (ANIM_SPEED * dt).min(1.0);
        // Ensure minimum speed so animation doesn't stall near the end
        let min_step = dt * 3.0;
        if step.abs() < min_step {
            anim + min_step.copysign(step)
        } else {
            anim + step
        }
        .clamp(0.0, 1.0)
    };
    unsafe {
        ANIM = new_anim;
    }

    // Request repaint while animating
    if (new_anim - target).abs() > 0.001 {
        ctx.request_repaint();
    }
}

/// Draw the templates drawer with slide animation and outside-click-to-close.
pub fn draw_drawer(ctx: &egui::Context, screen_rect: egui::Rect) {
    tick_animation(ctx);

    let anim = unsafe { ANIM };
    if anim < 0.005 {
        return; // fully closed, don't draw
    }

    let drawer_w = RIGHT_PANEL_WIDTH + PADDING * 2.0;
    // Open: ease_out (fast start, slow end). Close: mirror so speed profile is same but reversed.
    let eased = if unsafe { DRAWER_OPEN } {
        ease_out(anim)
    } else {
        1.0 - ease_out(1.0 - anim)
    };

    // Slide in from right: x offset from drawer_w (hidden) to 0 (visible)
    let x_offset = drawer_w * (1.0 - eased);
    let drawer_x = screen_rect.right() - drawer_w + x_offset;
    let drawer_rect = egui::Rect::from_min_size(
        Pos2::new(drawer_x, screen_rect.top()),
        Vec2::new(drawer_w, screen_rect.height()),
    );

    // Semi-transparent background opacity also fades in
    let bg_alpha = (eased * 200.0).min(200.0) as u8;

    // Pre-compute button rect for hit testing (includes protruding area)
    let btn_w = BUTTON_HEIGHT * 0.6;
    let btn_h = ICON_SIZE * 2.0;
    let btn_rect = egui::Rect::from_center_size(
        Pos2::new(drawer_rect.left(), drawer_rect.center().y),
        Vec2::new(btn_w, btn_h),
    );
    let hit_rect = if unsafe { DRAWER_OPEN } {
        drawer_rect.union(btn_rect)
    } else {
        drawer_rect
    };

    egui::Area::new(egui::Id::new("templates_drawer"))
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
                Stroke::new(1.0_f32, BORDER_LIGHT),
                StrokeKind::Outside,
            );

            // ── Pull-back button protruding from left edge, vertically centered ──
            // Only show while drawer is fully open; hide immediately on click
            if unsafe { DRAWER_OPEN } {
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
                    Stroke::new(1.0_f32, BUTTON_BORDER),
                    StrokeKind::Outside,
                );
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "▶",
                    egui::FontId::proportional(FONT_BODY),
                    TEXT_PRIMARY,
                );
                if resp.clicked() {
                    unsafe {
                        DRAWER_OPEN = false;
                    }
                }
            }

            // Detect outside click: close if clicked outside drawer + button area
            let input = ui.input(|i| (i.pointer.primary_clicked(), i.pointer.latest_pos()));
            if let (true, Some(pos)) = input {
                if !hit_rect.contains(pos) {
                    unsafe {
                        DRAWER_OPEN = false;
                    }
                }
            }

            // ── Content (offset by half button width since it protrudes) ──
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

                section_header(ui, "Templates");
                ui.add_space(SPACING * 1.5);

                template_item(ui, "Slide Pattern A", 3, true);
                template_item(ui, "Hold Sequence", 1, false);

                ui.add_space(SPACING);
                Button::new(
                    "+ New Template",
                    ButtonKind::Normal,
                    Vec2::new(ui.available_width(), BUTTON_HEIGHT),
                )
                .show(ui);
            });
        });
}

fn template_item(ui: &mut egui::Ui, name: &str, instance_count: usize, selected: bool) {
    let bg = if selected {
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
        Stroke::new(1.0_f32, BORDER_LIGHT),
        StrokeKind::Outside,
    );

    ui.painter().text(
        egui::pos2(btn.rect.left() + PADDING, btn.rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(FONT_BODY),
        TEXT_PRIMARY,
    );

    let count_text = format!("×{}", instance_count);
    ui.painter().text(
        egui::pos2(btn.rect.right() - PADDING, btn.rect.center().y),
        egui::Align2::RIGHT_CENTER,
        &count_text,
        egui::FontId::proportional(FONT_BODY),
        TEXT_DIM,
    );
}

// ══════════════════════════════════════════════════════════════
// Scene Hierarchy Drawer (slides from timeline right edge)
// ══════════════════════════════════════════════════════════════

/// Arrow button in timeline top-right area. Only visible when drawer is closed.
pub fn draw_scene_toggle_button(ui: &mut egui::Ui, timeline_rect: egui::Rect) {
    let anim = unsafe { SCENE_ANIM };
    if anim > 0.05 {
        return;
    }

    let btn_w = BUTTON_HEIGHT * 0.6;
    let btn_h = BUTTON_HEIGHT * 1.2;
    let btn_rect = egui::Rect::from_min_size(
        Pos2::new(timeline_rect.right() - btn_w, timeline_rect.top() + PADDING),
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
        Stroke::new(1.0_f32, BUTTON_BORDER),
        StrokeKind::Outside,
    );
    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "◀",
        egui::FontId::proportional(FONT_SMALL),
        TEXT_PRIMARY,
    );

    if resp.clicked() {
        unsafe {
            SCENE_DRAWER_OPEN = true;
        }
    }
}

fn tick_scene_animation(ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    let last = unsafe { SCENE_LAST_TIME };
    let dt = if last < 0.0 {
        0.016
    } else {
        (now - last).min(0.1) as f32
    };
    unsafe {
        SCENE_LAST_TIME = now;
    }

    let target = if unsafe { SCENE_DRAWER_OPEN } {
        1.0
    } else {
        0.0
    };
    let anim = unsafe { SCENE_ANIM };
    let new_anim = if (anim - target).abs() < 0.005 {
        target
    } else {
        let step = (target - anim) * (ANIM_SPEED * dt).min(1.0);
        let min_step = dt * 3.0;
        if step.abs() < min_step {
            anim + min_step.copysign(step)
        } else {
            anim + step
        }
        .clamp(0.0, 1.0)
    };
    unsafe {
        SCENE_ANIM = new_anim;
    }

    if (new_anim - target).abs() > 0.001 {
        ctx.request_repaint();
    }
}

/// Draw scene hierarchy drawer sliding from timeline right edge.
pub fn draw_scene_drawer(ctx: &egui::Context, timeline_rect: egui::Rect) {
    tick_scene_animation(ctx);

    let anim = unsafe { SCENE_ANIM };
    if anim < 0.005 {
        return;
    }

    let drawer_w = timeline_rect.width() / 3.0;
    let drawer_h = timeline_rect.height() * 0.6;
    let eased = if unsafe { SCENE_DRAWER_OPEN } {
        ease_out(anim)
    } else {
        1.0 - ease_out(1.0 - anim)
    };

    let x_offset = drawer_w * (1.0 - eased);
    let drawer_rect = egui::Rect::from_min_size(
        Pos2::new(
            timeline_rect.right() - drawer_w + x_offset,
            timeline_rect.top(),
        ),
        Vec2::new(drawer_w, drawer_h),
    );

    let bg_alpha = (eased * 200.0).min(200.0) as u8;

    // Pull-back button (left edge, matching template drawer)
    let btn_w = BUTTON_HEIGHT * 0.6;
    let btn_h = ICON_SIZE * 2.0;
    let btn_rect = egui::Rect::from_center_size(
        Pos2::new(drawer_rect.left(), drawer_rect.center().y),
        Vec2::new(btn_w, btn_h),
    );
    let hit_rect = if unsafe { SCENE_DRAWER_OPEN } {
        drawer_rect.union(btn_rect)
    } else {
        drawer_rect
    };

    egui::Area::new(egui::Id::new("scene_drawer"))
        .fixed_pos(drawer_rect.left_top())
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                drawer_rect,
                CornerRadius::same(6),
                Color32::from_rgba_premultiplied(42, 42, 46, bg_alpha),
            );
            ui.painter().rect_stroke(
                drawer_rect,
                CornerRadius::same(6),
                Stroke::new(1.0_f32, BORDER_LIGHT),
                StrokeKind::Outside,
            );

            // Pull-back button protruding from left edge
            if unsafe { SCENE_DRAWER_OPEN } {
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
                    Stroke::new(1.0_f32, BUTTON_BORDER),
                    StrokeKind::Outside,
                );
                ui.painter().text(
                    btn_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "▶",
                    egui::FontId::proportional(FONT_BODY),
                    TEXT_PRIMARY,
                );
                if resp.clicked() {
                    unsafe {
                        SCENE_DRAWER_OPEN = false;
                    }
                }
            }

            // Outside click to close
            let input = ui.input(|i| (i.pointer.primary_clicked(), i.pointer.latest_pos()));
            if let (true, Some(pos)) = input {
                if !hit_rect.contains(pos) {
                    unsafe {
                        SCENE_DRAWER_OPEN = false;
                    }
                }
            }

            // Content (offset by half button width since it protrudes)
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
                    .max_rect(content_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );

            child_ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = SPACING * 0.5;

                section_header(ui, "Scene");
                ui.add_space(SPACING);

                scene_tree_item(ui, "▼", "Notes", true, 0);
                scene_tree_item(ui, "  ▶", "Tap", false, 1);
                scene_tree_item(ui, "  ▶", "Hold", false, 1);
                scene_tree_item(ui, "  ▶", "Slide", false, 1);
                scene_tree_item(ui, "▶", "BPM Changes", false, 0);
                scene_tree_item(ui, "▶", "Camera", false, 0);
            });
        });
}

fn scene_tree_item(ui: &mut egui::Ui, icon: &str, name: &str, selected: bool, indent: usize) {
    let bg = if selected {
        Color32::from_rgb(39, 62, 93)
    } else {
        Color32::TRANSPARENT
    };
    let h = BUTTON_HEIGHT * 1.0;
    let indent_w = indent as f32 * SPACING * 3.0;

    let btn = ui.allocate_response(Vec2::new(ui.available_width(), h), egui::Sense::click());
    if bg != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(btn.rect, CornerRadius::same(3), bg);
    }
    if btn.hovered() && !selected {
        ui.painter().rect_filled(
            btn.rect,
            CornerRadius::same(3),
            Color32::from_rgba_premultiplied(255, 255, 255, 8),
        );
    }

    ui.painter().text(
        egui::pos2(btn.rect.left() + PADDING + indent_w, btn.rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} {}", icon, name),
        egui::FontId::proportional(FONT_SMALL),
        TEXT_PRIMARY,
    );
}
