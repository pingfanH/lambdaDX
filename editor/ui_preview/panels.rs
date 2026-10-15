use crate::{
    model::{Demo, Kind, LENGTH},
    theme::*,
};
use egui_macroquad::egui::*;
use lucide_icons::Icon;
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaneKind {
    Chart,
    Pad,
    Autoplay,
    Properties,
    Templates,
    History,
    Overview,
}

impl PaneKind {
    pub const ALL: [Self; 7] = [
        Self::Chart,
        Self::Pad,
        Self::Autoplay,
        Self::Properties,
        Self::Templates,
        Self::History,
        Self::Overview,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Chart => "Chart",
            Self::Pad => "Pad",
            Self::Autoplay => "Autoplay",
            Self::Properties => "Properties",
            Self::Templates => "Templates",
            Self::History => "History",
            Self::Overview => "Overview",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Chart => Icon::PanelsTopLeft,
            Self::Pad => Icon::Crosshair,
            Self::Autoplay => Icon::ScanLine,
            Self::Properties => Icon::Settings2,
            Self::Templates => Icon::Copy,
            Self::History => Icon::Undo2,
            Self::Overview => Icon::Focus,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pane {
    pub id: u64,
    pub kind: PaneKind,
    pub zoom: f32,
    pub center: f32,
    pub follow: bool,
    pub detail: bool,
}

impl Pane {
    pub fn new(id: u64, kind: PaneKind) -> Self {
        Self {
            id,
            kind,
            zoom: 1.0,
            center: 24.0,
            follow: true,
            detail: true,
        }
    }
}

pub fn body(ui: &mut Ui, pane: &mut Pane, demo: &mut Demo, assets: &Assets) {
    match pane.kind {
        PaneKind::Chart => chart(ui, pane, demo, assets),
        PaneKind::Pad => pad(ui, demo, assets),
        PaneKind::Autoplay => autoplay(ui, pane, demo),
        PaneKind::Properties => properties(ui, demo),
        PaneKind::Templates => templates(ui, demo, assets),
        PaneKind::History => history(ui, demo),
        PaneKind::Overview => overview(ui, demo),
    }
}

fn tex(assets: &Assets, kind: Kind, each: bool) -> &TextureHandle {
    match kind {
        Kind::Tap if each => &assets.each,
        Kind::Tap => &assets.tap,
        Kind::Hold => &assets.hold,
        Kind::Slide => &assets.star,
        Kind::Touch => &assets.touch,
    }
}

pub fn header_tools(ui: &mut Ui, pane: &mut Pane, demo: &mut Demo) {
    match pane.kind {
        PaneKind::Chart | PaneKind::Autoplay => {
            if ui.available_width() >= 320.0 {
                timeline_tools(ui, pane, demo);
            } else {
                if ui.available_width() >= 145.0 {
                    snap_tools(ui, pane, demo);
                }
                ui.menu_button(glyph(Icon::SlidersHorizontal, 14.0), |ui| {
                    ui.horizontal(|ui| timeline_tools(ui, pane, demo));
                })
                .response
                .on_hover_text("Timeline controls");
            }
        }
        PaneKind::Pad => {
            if icon(ui, Icon::Crosshair, "Sensor overlay", demo.show_sensors).clicked() {
                demo.show_sensors = !demo.show_sensors;
            }
            if ui.available_width() >= 65.0 {
                ui.label(RichText::new("NO CORE").size(10.0).color(GOLD));
            }
        }
        PaneKind::Properties => {
            if ui.available_width() >= 230.0 {
                if let Some(note) = demo.selected() {
                    ui.label(
                        RichText::new(format!("{} #{:04}", note.kind.label(), note.id))
                            .size(11.0)
                            .color(MUTED),
                    );
                }
            }
            if ui.available_width() >= 110.0 {
                note_actions(ui, demo);
            } else {
                ui.menu_button(glyph(Icon::SlidersHorizontal, 14.0), |ui| {
                    ui.horizontal(|ui| note_actions(ui, demo));
                })
                .response
                .on_hover_text("Note actions");
            }
        }
        PaneKind::History => {
            if ui.available_width() >= 55.0 {
                history_actions(ui, demo);
            } else {
                ui.menu_button(glyph(Icon::SlidersHorizontal, 14.0), |ui| {
                    ui.horizontal(|ui| history_actions(ui, demo));
                })
                .response
                .on_hover_text("History actions");
            }
        }
        _ => {}
    }
}

fn history_actions(ui: &mut Ui, demo: &mut Demo) {
    if icon(ui, Icon::Undo2, "Undo", false).clicked() {
        demo.undo();
    }
    if icon(ui, Icon::Redo2, "Redo", false).clicked() {
        demo.redo();
    }
}

fn snap_tools(ui: &mut Ui, pane: &Pane, demo: &mut Demo) {
    if icon(ui, Icon::Magnet, "Snap to grid", demo.snap).clicked() {
        demo.snap = !demo.snap;
    }
    ComboBox::from_id_salt((pane.id, "division"))
        .width(60.0)
        .selected_text(format!("1/{}", demo.division * 4))
        .show_ui(ui, |ui| {
            for v in [1, 2, 3, 4, 6, 8, 12, 16] {
                ui.selectable_value(&mut demo.division, v, format!("1/{}", v * 4));
            }
        });
}

fn timeline_tools(ui: &mut Ui, pane: &mut Pane, demo: &mut Demo) {
    snap_tools(ui, pane, demo);
    ui.separator();
    if icon(ui, Icon::ZoomOut, "Zoom out", false).clicked() {
        pane.zoom = (pane.zoom / 1.2).max(0.3);
    }
    let mut percentage = pane.zoom * 100.0;
    if ui
        .add(
            DragValue::new(&mut percentage)
                .range(30.0..=400.0)
                .suffix("%")
                .speed(1.0),
        )
        .changed()
    {
        pane.zoom = percentage / 100.0;
    }
    if icon(ui, Icon::ZoomIn, "Zoom in", false).clicked() {
        pane.zoom = (pane.zoom * 1.2).min(4.0);
    }
    ui.separator();
    if icon(ui, Icon::ScanLine, "Follow playhead", pane.follow).clicked() {
        pane.follow = !pane.follow;
        pane.center = demo.beat;
    }
    if icon(ui, Icon::Focus, "Frame selected note", false).clicked() {
        pane.center = demo.selected().map_or(demo.beat, |n| n.beat);
        pane.follow = false;
    }
}

fn note_actions(ui: &mut Ui, demo: &mut Demo) {
    ui.add_enabled_ui(demo.selected().is_some(), |ui| {
        if icon(ui, Icon::Copy, "Duplicate note", false).clicked() {
            demo.duplicate();
        }
        if icon(ui, Icon::Trash2, "Delete note", false).clicked() {
            demo.delete();
        }
        if icon(ui, Icon::ScanLine, "Seek to note", false).clicked() {
            if let Some(note) = demo.selected() {
                demo.beat = note.beat;
            }
        }
        if icon(ui, Icon::X, "Clear selection", false).clicked() {
            demo.select(None);
        }
    });
}

fn scroll(ui: &Ui, response: &Response, pane: &mut Pane, demo: &Demo, pan_rect: Rect) {
    if pane.follow {
        pane.center = demo.beat;
    }
    if !response.hovered() && !ui.rect_contains_pointer(response.rect) {
        return;
    }
    let (delta, command) =
        ui.input(|i| (i.raw_scroll_delta, i.modifiers.command || i.modifiers.ctrl));
    if command {
        // The center is a beat, never a cached pixel offset. Zoom cannot move the playhead.
        pane.center = demo.beat;
        pane.zoom = (pane.zoom * (delta.y * 0.012).exp()).clamp(0.3, 4.0);
    } else if delta != Vec2::ZERO && ui.rect_contains_pointer(pan_rect) {
        pane.follow = false;
        pane.center = (pane.center + (delta.y - delta.x) / (58.0 * pane.zoom)).clamp(0.0, LENGTH);
    }
}

fn hold_sprite(
    p: &Painter,
    texture: &TextureHandle,
    center: Pos2,
    width: f32,
    length: f32,
    alpha: f32,
) {
    let cap = width * 0.45;
    let tint = Color32::WHITE.gamma_multiply(alpha);
    let left = center.x - width / 2.0;
    let top = center.y - length - cap;
    for (y, height, v0, v1) in [
        (top, cap, 0.0, 0.4),
        (top + cap, length.max(1.0), 0.4, 0.6),
        (center.y, cap, 0.6, 1.0),
    ] {
        p.image(
            texture.id(),
            Rect::from_min_size(pos2(left, y), vec2(width, height)),
            Rect::from_min_max(pos2(0.0, v0), pos2(1.0, v1)),
            tint,
        );
    }
}

fn chart(ui: &mut Ui, pane: &mut Pane, demo: &mut Demo, assets: &Assets) {
    let (rect, response) = ui.allocate_exact_size(
        ui.available_size().max(vec2(1.0, 1.0)),
        Sense::click_and_drag(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0, CANVAS);
    let mut tool_ui = ui.new_child(
        UiBuilder::new()
            .id_salt((pane.id, "floating-tools"))
            .max_rect(Rect::from_min_size(
                rect.min + vec2(8.0, 8.0),
                vec2(34.0, (rect.height() - 16.0).max(1.0)),
            ))
            .layout(Layout::top_down(Align::Min)),
    );
    tool_ui.set_clip_rect(rect.intersect(ui.clip_rect()));
    let tools = Frame::new()
        .fill(glass(demo.translucent))
        .stroke(glass_edge())
        .corner_radius(4)
        .inner_margin(4)
        .show(&mut tool_ui, |ui| {
            ui.set_width(26.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.spacing_mut().button_padding = Vec2::splat(3.0);
            if icon(ui, Icon::MousePointer2, "Select", demo.tool.is_none()).clicked() {
                demo.tool = None;
            }
            ui.add_space(3.0);
            for kind in Kind::ALL {
                let image = Image::new((tex(assets, kind, false).id(), vec2(20.0, 20.0)));
                let response = ui
                    .add_sized(vec2(26.0, 26.0), Button::image(image).stroke(Stroke::NONE))
                    .on_hover_text(kind.label());
                if response.clicked() {
                    demo.tool = Some(kind);
                }
                if demo.tool == Some(kind) {
                    ui.painter().vline(
                        response.rect.left() - 2.0,
                        response.rect.center().y - 5.0..=response.rect.center().y + 5.0,
                        Stroke::new(2.0_f32, BLUE),
                    );
                }
            }
            ui.add_space(3.0);
            if icon(ui, Icon::Copy, "Duplicate selected note", false).clicked() {
                demo.duplicate();
            }
        })
        .response
        .rect;
    if !ui.rect_contains_pointer(tools) {
        scroll(ui, &response, pane, demo, rect);
    }
    let plot = Rect::from_min_max(
        pos2(rect.left() + 84.0, rect.top() + 26.0),
        pos2(rect.right() - 18.0, rect.bottom() - 12.0),
    );
    if plot.width() < 20.0 || plot.height() < 30.0 {
        return;
    }
    let paint = p.with_clip_rect(plot);
    let scale = 58.0 * pane.zoom;
    let anchor = plot.top() + plot.height() * 0.72;
    let y = |beat: f32| anchor - (beat - pane.center) * scale;
    let beat_at = |at: f32| pane.center + (anchor - at) / scale;
    let lane_width = plot.width() / 8.0;
    let x = |lane: u8| plot.left() + (lane as f32 + 0.5) * lane_width;
    let lane_at = |at: f32| ((at - plot.left()) / lane_width).floor().clamp(0.0, 7.0) as u8;
    for lane in 0..8 {
        let r = Rect::from_min_max(
            pos2(plot.left() + lane as f32 * lane_width, plot.top()),
            pos2(plot.left() + (lane + 1) as f32 * lane_width, plot.bottom()),
        );
        if lane % 2 == 1 {
            paint.rect_filled(r, 0, PANEL);
        }
        paint.vline(r.left(), r.y_range(), Stroke::new(1.0_f32, CONTROL));
        text(
            &p,
            pos2(x(lane), rect.top() + 12.0),
            Align2::CENTER_CENTER,
            lane + 1,
            12.0,
            MUTED,
        );
    }
    let min_beat = beat_at(plot.bottom()).max(0.0);
    let max_beat = beat_at(plot.top()).min(LENGTH);
    let subdivisions = if scale < 30.0 { 1 } else { demo.division };
    for step in (min_beat * subdivisions as f32).floor() as i32
        ..=(max_beat * subdivisions as f32).ceil() as i32
    {
        let b = step as f32 / subdivisions as f32;
        let strong = (b % 4.0).abs() < 0.001;
        let whole = step % subdivisions as i32 == 0;
        if demo.show_grid {
            paint.hline(
                plot.x_range(),
                y(b),
                Stroke::new(
                    1.0_f32,
                    if strong {
                        BORDER
                    } else if whole {
                        CONTROL
                    } else {
                        Color32::from_gray(44)
                    },
                ),
            );
        }
        if strong {
            text(
                &p,
                pos2(plot.left() - 9.0, y(b)),
                Align2::RIGHT_CENTER,
                format!("{:03}", (b / 4.0) as i32 + 1),
                11.0,
                MUTED,
            );
        }
    }
    let note_size = 35.0 * demo.note_scale;
    let mut ordered = demo.notes.clone();
    ordered.sort_by(|a, b| b.beat.total_cmp(&a.beat));
    let mut hit = false;
    for note in ordered {
        let center = pos2(x(note.lane), y(note.beat));
        let head = Rect::from_center_size(center, Vec2::splat(note_size));
        let selected = Some(note.id) == demo.selected;
        if note.kind == Kind::Hold {
            hold_sprite(
                &paint,
                &assets.hold,
                center,
                note_size,
                note.duration * scale,
                1.0,
            );
        } else if note.kind == Kind::Slide {
            let start = pos2(center.x, y(note.beat + 1.0));
            let end = pos2(x(note.end_lane), y(note.beat + note.duration));
            paint.line_segment(
                [center, start],
                Stroke::new(1.0_f32, MUTED.gamma_multiply(0.5)),
            );
            let count = (start.distance(end) / 20.0).max(2.0) as usize;
            for i in 0..count {
                let at = start.lerp(end, i as f32 / count as f32);
                sprite(
                    &paint,
                    &assets.slide,
                    Rect::from_center_size(at, vec2(22.0, 13.0)),
                    0.75,
                );
            }
            paint.circle_filled(end, 4.0, BLUE);
            if selected {
                paint.circle_stroke(end, 8.0, Stroke::new(1.0_f32, BLUE));
            }
            sprite(&paint, &assets.star, head, 1.0);
        } else {
            sprite(&paint, tex(assets, note.kind, note.each), head, 1.0);
        }
        if !plot.intersects(head) {
            continue;
        }
        if selected {
            paint.rect_stroke(
                head.expand(4.0),
                3,
                Stroke::new(1.5_f32, BLUE),
                StrokeKind::Outside,
            );
            text(
                &paint,
                head.right_top() + vec2(8.0, 0.0),
                Align2::LEFT_TOP,
                format!("#{}", note.id),
                10.0,
                BLUE,
            );
        }
        let r = ui.interact(
            head.intersect(plot),
            ui.id().with(("note", note.id)),
            Sense::click_and_drag(),
        );
        if r.hovered() {
            hit = true;
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
        if r.clicked() || r.drag_started() {
            demo.select(Some(note.id));
            demo.cursor = note.beat;
        }
        if r.drag_started() {
            demo.checkpoint("Move note");
        }
        if r.dragged() {
            if let Some(pos) = r.interact_pointer_pos() {
                let beat = demo.snapped(beat_at(pos.y));
                if let Some(n) = demo.notes.iter_mut().find(|n| n.id == note.id) {
                    n.beat = beat;
                    n.lane = lane_at(pos.x);
                }
            }
        }
        if r.secondary_clicked() {
            demo.select(Some(note.id));
            demo.delete();
        }
    }
    if let Some(pos) = response.hover_pos().filter(|pos| plot.contains(*pos)) {
        let beat = demo.snapped(beat_at(pos.y));
        let lane = lane_at(pos.x);
        if let Some(kind) = demo.tool {
            if !hit && !ui.input(|i| i.pointer.any_down()) {
                sprite(
                    &paint,
                    tex(assets, kind, false),
                    Rect::from_center_size(pos2(x(lane), y(beat)), Vec2::splat(note_size)),
                    0.42,
                );
                paint.hline(
                    plot.x_range(),
                    y(beat),
                    Stroke::new(1.0_f32, GOLD.gamma_multiply(0.28)),
                );
            }
        }
        if response.clicked() && !hit {
            demo.cursor = beat;
            if let Some(kind) = demo.tool {
                demo.place(kind, beat, lane);
            } else {
                demo.select(None);
            }
        }
    }
    if response.clicked() {
        if let Some(pos) = response
            .interact_pointer_pos()
            .filter(|p| p.x >= tools.right() && p.x < plot.left())
        {
            demo.beat = demo.snapped(beat_at(pos.y));
        }
    }
    paint.hline(
        plot.x_range(),
        y(demo.cursor),
        Stroke::new(1.0_f32, GOLD.gamma_multiply(0.6)),
    );
    paint.hline(plot.x_range(), y(demo.beat), Stroke::new(1.5_f32, RED));
    let play_tag =
        Rect::from_center_size(pos2(plot.right() - 24.0, y(demo.beat)), vec2(48.0, 18.0));
    paint.rect_filled(play_tag, 2, RED);
    text(
        &paint,
        play_tag.center(),
        Align2::CENTER_CENTER,
        format!("{:.2}", demo.beat),
        10.0,
        WINDOW,
    );
}

fn point(center: Pos2, radius: f32, lane: u8) -> Pos2 {
    let angle = -TAU / 4.0 + TAU / 16.0 + lane as f32 * TAU / 8.0;
    center + vec2(angle.cos(), angle.sin()) * radius
}

fn pad(ui: &mut Ui, demo: &mut Demo, assets: &Assets) {
    let (rect, _) =
        ui.allocate_exact_size(ui.available_size().max(Vec2::splat(1.0)), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0, WINDOW);
    let center = rect.center();
    let radius = (rect.width().min(rect.height()) * 0.44).max(10.0);
    p.circle_filled(center, radius, CANVAS);
    p.circle_stroke(center, radius, Stroke::new(2.0_f32, BORDER));
    p.circle_stroke(
        center,
        radius * 0.84,
        Stroke::new(1.0_f32, MUTED.gamma_multiply(0.36)),
    );
    p.circle_stroke(center, radius * 0.51, Stroke::new(1.0_f32, CONTROL));
    if demo.show_sensors {
        p.circle_stroke(
            center,
            radius * 0.23,
            Stroke::new(1.0_f32, GREEN.gamma_multiply(0.6)),
        );
        for lane in 0..8 {
            let a = point(center, radius * 0.26, lane);
            let b = point(center, radius * 0.82, lane);
            p.line_segment([a, b], Stroke::new(1.0_f32, GREEN.gamma_multiply(0.2)));
            text(
                &p,
                point(center, radius * 0.56, lane),
                Align2::CENTER_CENTER,
                format!("B{}", lane + 1),
                10.0,
                GREEN.gamma_multiply(0.8),
            );
        }
    }
    for lane in 0..8 {
        text(
            &p,
            point(center, radius * 0.94, lane),
            Align2::CENTER_CENTER,
            lane + 1,
            11.0,
            MUTED,
        );
        p.circle_filled(point(center, radius * 0.84, lane), 2.0, MUTED);
    }
    for note in &demo.notes {
        let dt = note.beat - demo.beat;
        let selected = Some(note.id) == demo.selected;
        if note.kind == Kind::Slide && dt < 4.0 && dt > -note.duration {
            let a = point(center, radius * 0.84, note.lane);
            let b = point(center, radius * 0.84, note.end_lane);
            let count = (a.distance(b) / 15.0) as usize;
            for i in 1..count {
                sprite(
                    &p,
                    &assets.slide,
                    Rect::from_center_size(a.lerp(b, i as f32 / count as f32), vec2(17.0, 11.0)),
                    if selected { 0.95 } else { 0.4 },
                );
            }
            if dt <= 0.0 {
                let pos = a.lerp(
                    b,
                    ((-dt - 1.0) / (note.duration - 1.0).max(0.2)).clamp(0.0, 1.0),
                );
                sprite(
                    &p,
                    &assets.star,
                    Rect::from_center_size(pos, Vec2::splat(30.0 * demo.note_scale)),
                    1.0,
                );
            }
        }
        if !(-0.2..=4.0).contains(&dt)
            && !(note.kind == Kind::Hold && dt <= 0.0 && dt > -note.duration)
        {
            continue;
        }
        let r = if note.kind == Kind::Touch {
            radius * 0.5
        } else {
            radius * (0.25 + 0.59 * (1.0 - dt / 4.0).clamp(0.0, 1.0))
        };
        let at = point(center, r, note.lane);
        let size = 29.0 * demo.note_scale;
        sprite(
            &p,
            tex(assets, note.kind, note.each),
            Rect::from_center_size(at, Vec2::splat(size)),
            1.0,
        );
        if selected {
            p.circle_stroke(at, size * 0.65, Stroke::new(1.0_f32, BLUE));
        }
    }
    text(
        &p,
        pos2(rect.left() + 12.0, rect.bottom() - 12.0),
        Align2::LEFT_BOTTOM,
        format!("{:06.2} s", demo.beat * 60.0 / demo.bpm),
        11.0,
        MUTED,
    );
}

fn autoplay(ui: &mut Ui, pane: &mut Pane, demo: &mut Demo) {
    let (rect, response) = ui.allocate_exact_size(
        ui.available_size().max(Vec2::splat(1.0)),
        Sense::click_and_drag(),
    );
    let row_h = 43.0;
    let ruler_h = 27.0;
    let detail_y = rect.top() + ruler_h + row_h * 4.0 + 12.0;
    let pan_rect = if pane.detail && demo.selected().is_some() {
        Rect::from_min_max(rect.min, pos2(rect.right(), detail_y))
    } else {
        rect
    };
    scroll(ui, &response, pane, demo, pan_rect);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0, CANVAS);
    let label_width = 104.0_f32.min(rect.width() * 0.2);
    let plot = Rect::from_min_max(pos2(rect.left() + label_width, rect.top()), rect.max);
    if plot.width() < 50.0 {
        return;
    }
    let paint = p.with_clip_rect(plot);
    let scale = 84.0 * pane.zoom;
    let x = |beat: f32| plot.center().x + (beat - pane.center) * scale;
    let beat_at = |at: f32| pane.center + (at - plot.center().x) / scale;
    for row in 0..4 {
        let y = rect.top() + ruler_h + row as f32 * row_h;
        p.rect_filled(
            Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), row_h)),
            0,
            if row % 2 == 0 { CANVAS } else { PANEL },
        );
        text(
            &p,
            pos2(rect.left() + 14.0, y + row_h * 0.5),
            Align2::LEFT_CENTER,
            format!("Track {:02}", row + 1),
            12.0,
            MUTED,
        );
        p.hline(rect.x_range(), y + row_h, Stroke::new(1.0_f32, CONTROL));
    }
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), ruler_h)),
        0,
        PANEL,
    );
    let first = beat_at(plot.left()).floor().max(0.0) as i32;
    let last = beat_at(plot.right()).ceil().min(LENGTH) as i32;
    for b in first..=last {
        let at = x(b as f32);
        paint.vline(
            at,
            rect.y_range(),
            Stroke::new(1.0_f32, if b % 4 == 0 { BORDER } else { CONTROL }),
        );
        text(
            &paint,
            pos2(at + 6.0, rect.top() + 13.0),
            Align2::LEFT_CENTER,
            if b % 4 == 0 {
                format!("{:03}", b / 4 + 1)
            } else {
                format!(".{}", b % 4 + 1)
            },
            10.0,
            MUTED,
        );
    }
    let mut ends = [f32::NEG_INFINITY; 4];
    let mut notes = demo.notes.clone();
    notes.sort_by(|a, b| a.beat.total_cmp(&b.beat).then(a.id.cmp(&b.id)));
    let mut hit = false;
    for note in notes {
        let track = ends.iter().position(|end| *end <= note.beat).unwrap_or(3);
        ends[track] = note.end();
        let top = rect.top() + ruler_h + track as f32 * row_h + 7.0;
        let clip = Rect::from_min_max(
            pos2(x(note.beat), top),
            pos2(x(note.end()).max(x(note.beat) + 17.0), top + row_h - 14.0),
        );
        let selected = demo.selected == Some(note.id);
        let color = note.kind.color();
        paint.rect_filled(
            clip,
            3,
            color.gamma_multiply(if selected { 0.3 } else { 0.14 }),
        );
        paint.rect_stroke(
            clip,
            3,
            Stroke::new(
                if selected { 1.5_f32 } else { 1.0_f32 },
                color.gamma_multiply(if selected { 1.0 } else { 0.55 }),
            ),
            StrokeKind::Inside,
        );
        paint.vline(
            clip.left() + 3.0,
            (clip.top() + 4.0)..=(clip.bottom() - 4.0),
            Stroke::new(2.0_f32, color),
        );
        if clip.width() > 58.0 {
            text(
                &paint,
                clip.left_center() + vec2(11.0, 0.0),
                Align2::LEFT_CENTER,
                format!("{}  #{}", note.kind.label(), note.id),
                11.0,
                TEXT,
            );
        }
        if !plot.intersects(clip) {
            continue;
        }
        let r = ui.interact(
            clip.intersect(plot),
            ui.id().with(("clip", note.id)),
            Sense::click_and_drag(),
        );
        hit |= r.hovered();
        if r.clicked() || r.drag_started() {
            demo.select(Some(note.id));
        }
        if r.drag_started() {
            demo.checkpoint("Move autoplay clip");
        }
        if r.dragged() {
            let delta = ui.input(|i| i.pointer.delta().x) / scale;
            if let Some(n) = demo.notes.iter_mut().find(|n| n.id == note.id) {
                n.beat = (n.beat + delta).clamp(0.0, LENGTH);
            }
        }
        if r.drag_stopped() {
            let beat = demo
                .notes
                .iter()
                .find(|n| n.id == note.id)
                .map(|n| demo.snapped(n.beat));
            if let (Some(n), Some(beat)) = (demo.notes.iter_mut().find(|n| n.id == note.id), beat) {
                n.beat = beat;
            }
        }
    }
    if pane.detail {
        if let Some(note) = demo.selected().cloned() {
            let header = Rect::from_min_size(pos2(rect.left(), detail_y), vec2(rect.width(), 28.0));
            p.rect_filled(header, 0, PANEL);
            text(
                &p,
                header.left_center() + vec2(13.0, 0.0),
                Align2::LEFT_CENTER,
                format!(
                    "{} #{}  /  {} sensor intervals",
                    note.kind.label(),
                    note.id,
                    note.events.len()
                ),
                12.0,
                TEXT,
            );
            let detail_rect = Rect::from_min_max(header.left_bottom(), rect.max);
            if detail_rect.height() > 0.0 {
                let mut detail_ui = ui.new_child(
                    UiBuilder::new()
                        .id_salt((pane.id, "sensor-details"))
                        .max_rect(detail_rect),
                );
                detail_ui.set_clip_rect(detail_rect.intersect(ui.clip_rect()));
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(&mut detail_ui, |ui| {
                        let (content, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), note.events.len() as f32 * 30.0 + 5.0),
                            Sense::hover(),
                        );
                        let p = ui.painter();
                        let paint = p.with_clip_rect(plot.intersect(ui.clip_rect()));
                        for (index, event) in note.events.iter().enumerate() {
                            let top = content.top() + index as f32 * 30.0 + 5.0;
                            text(
                                p,
                                pos2(rect.left() + 22.0, top + 10.0),
                                Align2::LEFT_CENTER,
                                &event.sensor,
                                11.0,
                                MUTED,
                            );
                            let span = Rect::from_min_max(
                                pos2(x(note.beat + event.start), top),
                                pos2(x(note.beat + event.end), top + 20.0),
                            );
                            let active = demo.selected_event == Some(index);
                            paint.rect_filled(
                                span,
                                2,
                                GREEN.gamma_multiply(if active { 0.48 } else { 0.24 }),
                            );
                            paint.rect_stroke(
                                span,
                                2,
                                Stroke::new(1.0_f32, GREEN),
                                StrokeKind::Inside,
                            );
                            for (end, at) in [(false, span.left()), (true, span.right())] {
                                let handle = Rect::from_center_size(
                                    pos2(at, span.center().y),
                                    vec2(8.0, 24.0),
                                );
                                paint.rect_filled(handle.shrink2(vec2(2.0, 3.0)), 1, GREEN);
                                let hit_rect = handle.intersect(plot).intersect(ui.clip_rect());
                                if !hit_rect.is_positive() {
                                    continue;
                                }
                                let r = ui
                                    .interact(
                                        hit_rect,
                                        ui.id().with(("event", note.id, index, end)),
                                        Sense::click_and_drag(),
                                    )
                                    .on_hover_cursor(CursorIcon::ResizeHorizontal);
                                hit |= r.hovered();
                                if r.clicked() || r.drag_started() {
                                    demo.selected_event = Some(index);
                                }
                                if r.drag_started() {
                                    demo.checkpoint("Resize sensor interval");
                                }
                                if r.dragged() {
                                    if let Some(pos) = r.interact_pointer_pos() {
                                        let beat = demo.snapped(beat_at(pos.x)) - note.beat;
                                        if let Some(n) =
                                            demo.notes.iter_mut().find(|n| n.id == note.id)
                                        {
                                            let e = &mut n.events[index];
                                            if end {
                                                e.end = beat.max(e.start + 0.05);
                                            } else {
                                                e.start = beat.clamp(0.0, (e.end - 0.05).max(0.0));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
            }
        }
    }
    if response.clicked() && !hit {
        if let Some(pos) = response.interact_pointer_pos() {
            if pos.y < rect.top() + ruler_h {
                demo.beat = demo.snapped(beat_at(pos.x));
            } else {
                demo.select(None);
            }
        }
    }
    if response.dragged() {
        if let Some(origin) = ui
            .input(|i| i.pointer.press_origin())
            .filter(|p| p.y < rect.top() + ruler_h)
        {
            let _ = origin;
            if let Some(pos) = response.interact_pointer_pos() {
                demo.beat = demo.snapped(beat_at(pos.x));
            }
        }
    }
    paint.vline(x(demo.beat), rect.y_range(), Stroke::new(1.5_f32, RED));
    paint.rect_filled(
        Rect::from_center_size(pos2(x(demo.beat), rect.top() + 4.0), vec2(10.0, 8.0)),
        2,
        RED,
    );
}

fn properties(ui: &mut Ui, demo: &mut Demo) {
    ScrollArea::vertical().show(ui, |ui| {
        Frame::new().inner_margin(10).show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 1.0).max(0.0));
            if let Some(mut note) = demo.selected().cloned() {
                let mut changed = false;
                Grid::new(ui.id().with("fields"))
                    .num_columns(2)
                    .spacing(vec2(14.0, 6.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new("Beat").color(MUTED));
                        changed |= ui
                            .add(
                                DragValue::new(&mut note.beat)
                                    .speed(0.25)
                                    .range(0.0..=LENGTH)
                                    .fixed_decimals(2),
                            )
                            .changed();
                        ui.end_row();
                        ui.label(RichText::new("Lane").color(MUTED));
                        let mut lane = note.lane + 1;
                        changed |= ui.add(DragValue::new(&mut lane).range(1..=8)).changed();
                        note.lane = lane - 1;
                        ui.end_row();
                        if matches!(note.kind, Kind::Hold | Kind::Slide) {
                            ui.label(RichText::new("Duration").color(MUTED));
                            changed |= ui
                                .add(
                                    DragValue::new(&mut note.duration)
                                        .speed(0.25)
                                        .range(0.0..=32.0)
                                        .suffix(" beats"),
                                )
                                .changed();
                            ui.end_row();
                        }
                        if note.kind == Kind::Slide {
                            ui.label(RichText::new("End lane").color(MUTED));
                            let mut lane = note.end_lane + 1;
                            changed |= ui.add(DragValue::new(&mut lane).range(1..=8)).changed();
                            note.end_lane = lane - 1;
                            ui.end_row();
                        }
                    });
                if changed {
                    demo.checkpoint("Edit note properties");
                    if let Some(n) = demo.notes.iter_mut().find(|n| n.id == note.id) {
                        *n = note.clone();
                    }
                }
                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("AUTOPLAY").size(10.0).color(MUTED));
                ui.horizontal(|ui| {
                    ui.label(format!("{} intervals", note.events.len()));
                    ui.label(RichText::new("Note-owned").color(GREEN).size(11.0));
                });
                for (i, event) in note.events.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(demo.selected_event == Some(i), &event.sensor)
                            .clicked()
                        {
                            demo.selected_event = Some(i);
                        }
                        ui.label(
                            RichText::new(format!("{:.2} - {:.2}", event.start, event.end))
                                .monospace()
                                .color(MUTED),
                        );
                    });
                }
            } else {
                ui.heading("Chart");
                ui.label(RichText::new("Workbench study").color(MUTED));
                ui.separator();
                ui.label(format!("{} notes", demo.notes.len()));
                ui.label(format!("{} BPM", demo.bpm));
                ui.label(RichText::new("No selection").color(MUTED));
            }
        });
    });
}

fn templates(ui: &mut Ui, demo: &mut Demo, assets: &Assets) {
    ScrollArea::vertical().show(ui, |ui| {
        Frame::new().inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("LOCAL PATTERNS").size(10.0).color(MUTED));
            for (name, kind, count) in [
                ("Alternating taps", Kind::Tap, 4),
                ("Touch staircase", Kind::Touch, 4),
                ("Opposite slides", Kind::Slide, 2),
            ] {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add(Image::new((
                        tex(assets, kind, false).id(),
                        vec2(26.0, 26.0),
                    )));
                    ui.vertical(|ui| {
                        ui.label(name);
                        ui.label(
                            RichText::new(format!("{count} notes"))
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon(ui, Icon::Plus, "Insert at edit cursor", false).clicked() {
                            for index in 0..count {
                                demo.place(kind, demo.cursor + index as f32, (index * 2) as u8);
                            }
                        }
                    });
                });
            }
        });
    });
}

fn history(ui: &mut Ui, demo: &mut Demo) {
    Frame::new().inner_margin(10).show(ui, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            for (i, label) in demo.history.iter().enumerate().rev() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:02}", i)).monospace().color(MUTED));
                    ui.label(label);
                });
            }
        });
    });
}

fn overview(ui: &mut Ui, demo: &mut Demo) {
    let (rect, response) = ui.allocate_exact_size(
        ui.available_size().max(Vec2::splat(1.0)),
        Sense::click_and_drag(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0, WINDOW);
    let r = rect.shrink2(vec2(16.0, 8.0));
    let x = |b: f32| r.left() + b / LENGTH * r.width();
    let rows = (r.height() - 20.0).max(8.0);
    for n in &demo.notes {
        p.rect_filled(
            Rect::from_min_size(
                pos2(x(n.beat), r.top() + n.lane as f32 * rows / 8.0),
                vec2((n.duration / LENGTH * r.width()).max(3.0), 3.0),
            ),
            0,
            n.kind.color().gamma_multiply(0.7),
        );
    }
    for b in (0..=64).step_by(8) {
        text(
            &p,
            pos2(x(b as f32), r.bottom()),
            Align2::CENTER_BOTTOM,
            format!("{:03}", b / 4 + 1),
            10.0,
            MUTED,
        );
    }
    let band = Rect::from_min_max(
        pos2(x((demo.beat - 2.0).max(0.0)), r.top()),
        pos2(x((demo.beat + 8.0).min(LENGTH)), r.bottom() - 15.0),
    );
    p.rect_stroke(
        band,
        2,
        Stroke::new(1.0_f32, MUTED.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    p.vline(x(demo.beat), r.y_range(), Stroke::new(1.5_f32, RED));
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            demo.beat = ((pos.x - r.left()) / r.width() * LENGTH).clamp(0.0, LENGTH);
        }
    }
}
