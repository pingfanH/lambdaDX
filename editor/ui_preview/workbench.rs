use crate::{
    model::Demo,
    panels::{self, Pane, PaneKind},
    theme::{self, *},
};
use egui_dock::{DockArea, DockState, NodeIndex, Split, SurfaceIndex, TabIndex, TabViewer};
use egui_macroquad::egui::*;
use lucide_icons::Icon;

const WORKSPACES: [&str; 3] = ["Chart", "Autoplay", "Combined"];

pub struct Workbench {
    demo: Demo,
    assets: Assets,
    docks: Vec<DockState<Pane>>,
    saved: Vec<Option<DockState<Pane>>>,
    workspace: usize,
    next_id: u64,
    maximized: Option<Pane>,
    settings: bool,
    settings_page: usize,
    layout_locked: bool,
    status: String,
}

enum Action {
    Focus(u64),
    Replace(u64, PaneKind),
    Split(u64, Split, PaneKind),
    Swap(u64, u64),
    Close(u64),
    Maximize(Pane),
    Restore,
}

fn preset(workspace: usize, next_id: &mut u64) -> DockState<Pane> {
    let mut pane = |kind| {
        *next_id += 1;
        Pane::new(*next_id, kind)
    };
    match workspace {
        1 => {
            let mut dock = DockState::new(vec![pane(PaneKind::Pad)]);
            let [top, _] = dock.main_surface_mut().split_below(
                NodeIndex::root(),
                0.36,
                vec![pane(PaneKind::Autoplay)],
            );
            dock.main_surface_mut()
                .split_right(top, 0.54, vec![pane(PaneKind::Properties)]);
            dock
        }
        2 => {
            let mut dock = DockState::new(vec![pane(PaneKind::Chart)]);
            let [top, _] = dock.main_surface_mut().split_below(
                NodeIndex::root(),
                0.52,
                vec![pane(PaneKind::Autoplay)],
            );
            dock.main_surface_mut()
                .split_right(top, 0.68, vec![pane(PaneKind::Pad)]);
            dock
        }
        _ => {
            let mut dock = DockState::new(vec![pane(PaneKind::Chart)]);
            let [main, right] = dock.main_surface_mut().split_right(
                NodeIndex::root(),
                0.72,
                vec![pane(PaneKind::Pad)],
            );
            dock.main_surface_mut()
                .split_below(main, 0.90, vec![pane(PaneKind::Overview)]);
            dock.main_surface_mut()
                .split_below(right, 0.46, vec![pane(PaneKind::Properties)]);
            dock
        }
    }
}

impl Workbench {
    pub fn new(ctx: &Context) -> Self {
        let mut next_id = 0;
        let docks = (0..3).map(|i| preset(i, &mut next_id)).collect();
        let workspace = match std::env::var("MAI2_UI_PREVIEW_WORKSPACE").as_deref() {
            Ok("autoplay") => 1,
            Ok("combined") => 2,
            _ => 0,
        };
        Self {
            demo: Demo::default(),
            assets: Assets::new(ctx),
            docks,
            saved: vec![None; 3],
            workspace,
            next_id,
            maximized: None,
            settings: std::env::var_os("MAI2_UI_PREVIEW_SETTINGS").is_some(),
            settings_page: 0,
            layout_locked: false,
            status: "Demo session".into(),
        }
    }

    pub fn show(&mut self, ctx: &Context, dt: f32) {
        self.demo.tick(dt);
        ctx.style_mut(|style| {
            style.visuals.window_fill = floating_glass(self.demo.translucent);
        });
        if !ctx.wants_keyboard_input() && !self.settings {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) {
                self.demo.playing = !self.demo.playing;
            }
            if ctx.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Delete)
                    || i.consume_key(Modifiers::NONE, Key::Backspace)
            }) {
                self.demo.delete();
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)) {
                self.demo.redo();
            } else if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z)) {
                self.demo.undo();
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::D)) {
                self.demo.duplicate();
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab)) {
                self.demo.next(true);
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
                self.demo.next(false);
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.restore_maximized();
                self.demo.tool = None;
                self.demo.select(None);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F1)) {
            self.settings = !self.settings;
        }
        TopBottomPanel::top("global-menu")
            .frame(
                Frame::new()
                    .fill(HEADER)
                    .inner_margin(Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                self.menu(ui);
            });
        TopBottomPanel::bottom("session-status")
            .frame(
                Frame::new()
                    .fill(WINDOW)
                    .inner_margin(Margin::symmetric(8, 1)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).size(11.0).color(MUTED));
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{} notes", self.demo.notes.len()))
                            .size(11.0)
                            .color(MUTED),
                    );
                    if let Some(n) = self.demo.selected() {
                        ui.label(
                            RichText::new(format!(
                                "{} #{}  /  Lane {}",
                                n.kind.label(),
                                n.id,
                                n.lane + 1
                            ))
                            .size(11.0)
                            .color(MUTED),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("UI Preview").size(10.0).color(MUTED));
                    });
                });
            });
        let mut actions = Vec::new();
        let areas: Vec<_> = self.docks[self.workspace]
            .iter_all_tabs()
            .map(|(_, pane)| (pane.id, pane.kind))
            .collect();
        CentralPanel::default()
            .frame(Frame::new().fill(WINDOW))
            .show(ctx, |ui| {
                if let Some(pane) = &mut self.maximized {
                    let (_, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                    area_header(
                        pane,
                        &mut self.demo,
                        &response,
                        &mut actions,
                        &areas,
                        self.layout_locked,
                        true,
                    );
                    ui.push_id(pane.id, |ui| {
                        panels::body(ui, pane, &mut self.demo, &self.assets)
                    });
                } else {
                    let style = theme::dock_style(ui.style());
                    let mut viewer = Viewer {
                        demo: &mut self.demo,
                        assets: &self.assets,
                        actions: &mut actions,
                        areas: &areas,
                        locked: self.layout_locked,
                    };
                    DockArea::new(&mut self.docks[self.workspace])
                        .id(Id::new(("workbench-dock", self.workspace)))
                        .style(style)
                        .show_add_buttons(false)
                        .show_close_buttons(false)
                        .show_leaf_close_all_buttons(false)
                        .show_leaf_collapse_buttons(false)
                        .draggable_tabs(false)
                        .tab_context_menus(false)
                        .show_inside(ui, &mut viewer);
                }
            });
        for action in actions {
            self.apply_action(action);
        }
        self.settings(ctx);
    }

    fn location(&self, id: u64) -> Option<(SurfaceIndex, NodeIndex)> {
        self.docks[self.workspace]
            .iter_all_tabs()
            .find_map(|(at, pane)| (pane.id == id).then_some(at))
    }

    fn apply_action(&mut self, action: Action) {
        match action {
            Action::Maximize(pane) => self.maximized = Some(pane),
            Action::Restore => self.restore_maximized(),
            Action::Focus(id) => {
                if let Some(at) = self.location(id) {
                    self.docks[self.workspace].set_focused_node_and_surface(at);
                }
            }
            _ if self.layout_locked => {}
            action => {
                self.restore_maximized();
                // Resolve stable pane IDs after each mutation: splits and closes reindex the tree.
                match action {
                    Action::Replace(id, kind) => {
                        if let Some((_, pane)) = self.docks[self.workspace]
                            .iter_all_tabs_mut()
                            .find(|(_, pane)| pane.id == id)
                        {
                            pane.kind = kind;
                        }
                    }
                    Action::Split(id, direction, kind) => {
                        if let Some((surface, node)) = self.location(id) {
                            self.next_id += 1;
                            let mut pane =
                                self.docks[self.workspace][surface][node].tabs().unwrap()[0]
                                    .clone();
                            pane.id = self.next_id;
                            pane.kind = kind;
                            let [_, added] = self.docks[self.workspace][surface].split_tabs(
                                node,
                                direction,
                                0.5,
                                vec![pane],
                            );
                            self.docks[self.workspace]
                                .set_focused_node_and_surface((surface, added));
                        }
                    }
                    Action::Swap(first, second) => {
                        if let (Some((sa, na)), Some((sb, nb))) =
                            (self.location(first), self.location(second))
                        {
                            let a = self.docks[self.workspace][sa][na].tabs().unwrap()[0].clone();
                            let b = self.docks[self.workspace][sb][nb].tabs().unwrap()[0].clone();
                            self.docks[self.workspace][sa][na].tabs_mut().unwrap()[0] = b;
                            self.docks[self.workspace][sb][nb].tabs_mut().unwrap()[0] = a;
                            self.docks[self.workspace].set_focused_node_and_surface((sb, nb));
                        }
                    }
                    Action::Close(id) if self.docks[self.workspace].iter_all_tabs().count() > 1 => {
                        if let Some((surface, node)) = self.location(id) {
                            self.docks[self.workspace].remove_tab((surface, node, TabIndex(0)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn add_area(&mut self, kind: PaneKind) {
        self.restore_maximized();
        let dock = &self.docks[self.workspace];
        let focused = dock.focused_leaf();
        let id = dock
            .iter_all_tabs()
            .find(|(at, _)| Some(*at) == focused)
            .or_else(|| dock.iter_all_tabs().next())
            .map(|(_, pane)| pane.id);
        if let Some(id) = id {
            self.apply_action(Action::Split(id, Split::Right, kind));
        }
    }

    fn restore_maximized(&mut self) {
        if let Some(pane) = self.maximized.take() {
            if let Some((_, original)) = self.docks[self.workspace]
                .iter_all_tabs_mut()
                .find(|(_, p)| p.id == pane.id)
            {
                *original = pane;
            }
        }
    }

    fn reset_layout(&mut self) {
        self.maximized = None;
        self.docks[self.workspace] = preset(self.workspace, &mut self.next_id);
        self.status = format!("{} layout restored", WORKSPACES[self.workspace]);
    }

    fn menu(&mut self, ui: &mut Ui) {
        menu::bar(ui, |ui| {
            let width = ui.available_width();
            ui.set_height(24.0);
            ui.label(RichText::new("lambdaDX").strong().size(13.0));
            ui.add_space(6.0);
            ui.menu_button("File", |ui| {
                if ui.button("Reset demo session").clicked() {
                    self.demo = Demo::default();
                    self.status = "Demo session reset".into();
                    ui.close_menu();
                }
                ui.separator();
                ui.add_enabled(false, Button::new("Save chart"));
                ui.add_enabled(false, Button::new("Export chart"));
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo").clicked() {
                    self.demo.undo();
                    ui.close_menu();
                }
                if ui.button("Redo").clicked() {
                    self.demo.redo();
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(self.demo.selected.is_some(), Button::new("Duplicate"))
                    .clicked()
                {
                    self.demo.duplicate();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(self.demo.selected.is_some(), Button::new("Delete"))
                    .clicked()
                {
                    self.demo.delete();
                    ui.close_menu();
                }
                if ui.button("Deselect").clicked() {
                    self.demo.select(None);
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                ui.add_enabled_ui(!self.layout_locked, |ui| {
                    ui.menu_button("Add editor area", |ui| {
                        for kind in PaneKind::ALL {
                            if ui.button(kind.title()).clicked() {
                                self.add_area(kind);
                                ui.close_menu();
                            }
                        }
                    });
                });
                if ui.button("Maximize active editor").clicked() {
                    if let Some((_, pane)) = self.docks[self.workspace].find_active_focused() {
                        self.maximized = Some(pane.clone());
                    }
                    ui.close_menu();
                }
                if ui.button("Restore layout").clicked() {
                    self.restore_maximized();
                    ui.close_menu();
                }
                ui.separator();
                ui.checkbox(&mut self.demo.show_grid, "Chart grid");
                ui.checkbox(&mut self.demo.show_sensors, "Pad sensors");
            });
            ui.menu_button("Playback", |ui| {
                if ui
                    .button(if self.demo.playing { "Pause" } else { "Play" })
                    .clicked()
                {
                    self.demo.playing = !self.demo.playing;
                    ui.close_menu();
                }
                if ui.button("Restart").clicked() {
                    self.demo.beat = 0.0;
                    ui.close_menu();
                }
                ui.checkbox(&mut self.demo.looping, "Loop");
                ui.separator();
                ui.horizontal(|ui| self.beat_control(ui));
                ui.horizontal(|ui| {
                    ui.label("Speed");
                    self.speed_control(ui);
                });
            });
            ui.add_space(16.0);
            self.workspace_controls(ui, width);
        });
    }

    fn workspace_controls(&mut self, ui: &mut Ui, width: f32) {
        if width >= 1040.0 {
            for (index, label) in WORKSPACES.iter().enumerate() {
                let active = self.workspace == index;
                let r = ui.button(RichText::new(*label).color(if active { TEXT } else { MUTED }));
                if r.clicked() {
                    self.restore_maximized();
                    self.workspace = index;
                }
                if self.workspace == index {
                    ui.painter().hline(
                        r.rect.x_range(),
                        r.rect.bottom() + 2.0,
                        Stroke::new(1.0_f32, BLUE),
                    );
                }
            }
        } else {
            ComboBox::from_id_salt("workspace")
                .width(94.0)
                .selected_text(WORKSPACES[self.workspace])
                .show_ui(ui, |ui| {
                    for (index, label) in WORKSPACES.iter().enumerate() {
                        if ui
                            .selectable_label(self.workspace == index, *label)
                            .clicked()
                        {
                            self.restore_maximized();
                            self.workspace = index;
                        }
                    }
                })
                .response
                .on_hover_text("Workspace");
        }
        let playback_width = 290.0
            + if width >= 1160.0 { 86.0 } else { 0.0 }
            + if width >= 960.0 { 75.0 } else { 0.0 };
        ui.add_space((ui.available_width() - playback_width).max(8.0));
        if icon(ui, Icon::SkipBack, "Restart", false).clicked() {
            self.demo.beat = 0.0;
        }
        if icon(
            ui,
            if self.demo.playing {
                Icon::Pause
            } else {
                Icon::Play
            },
            "Play / pause",
            self.demo.playing,
        )
        .clicked()
        {
            self.demo.playing = !self.demo.playing;
        }
        if icon(ui, Icon::Repeat2, "Loop", self.demo.looping).clicked() {
            self.demo.looping = !self.demo.looping;
        }
        let seconds = self.demo.beat * 60.0 / self.demo.bpm;
        ui.add_sized(
            vec2(92.0, 24.0),
            Label::new(
                RichText::new(format!(
                    "{:02}:{:06.3}",
                    (seconds / 60.0) as i32,
                    seconds % 60.0
                ))
                .monospace(),
            ),
        );
        if width >= 1160.0 {
            self.beat_control(ui);
        }
        if width >= 960.0 {
            self.speed_control(ui);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon(ui, Icon::Settings2, "Settings", self.settings).clicked() {
                self.settings = !self.settings;
            }
            ui.menu_button(theme::glyph(Icon::PanelsTopLeft, 16.0), |ui| {
                if ui.button("Remember layout").clicked() {
                    self.restore_maximized();
                    self.saved[self.workspace] = Some(self.docks[self.workspace].clone());
                    self.status = "Layout remembered for this session".into();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        self.saved[self.workspace].is_some(),
                        Button::new("Restore remembered layout"),
                    )
                    .clicked()
                {
                    self.maximized = None;
                    self.docks[self.workspace] =
                        self.saved[self.workspace].as_ref().unwrap().clone();
                    self.status = "Layout restored".into();
                    ui.close_menu();
                }
                if ui.button("Reset workspace layout").clicked() {
                    self.reset_layout();
                    ui.close_menu();
                }
                ui.separator();
                ui.checkbox(&mut self.layout_locked, "Lock layout");
            })
            .response
            .on_hover_text("Workspace layout");
        });
    }

    fn beat_control(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Beat").size(11.0).color(MUTED));
        ui.add(
            DragValue::new(&mut self.demo.beat)
                .range(0.0..=64.0)
                .speed(0.25)
                .fixed_decimals(2),
        );
    }

    fn speed_control(&mut self, ui: &mut Ui) {
        ComboBox::from_id_salt("speed")
            .width(65.0)
            .selected_text(format!("{:.2}x", self.demo.speed))
            .show_ui(ui, |ui| {
                for speed in [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
                    ui.selectable_value(&mut self.demo.speed, speed, format!("{speed:.2}x"));
                }
            });
    }

    fn settings(&mut self, ctx: &Context) {
        if !self.settings {
            return;
        }
        let mut open = true;
        Window::new("Settings")
            .id(Id::new("preview-settings"))
            .open(&mut open)
            .collapsible(false)
            .default_width(620.0)
            .default_height(360.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(135.0);
                        for (i, title) in ["Appearance", "Timeline", "Workspace"].iter().enumerate()
                        {
                            let active = self.settings_page == i;
                            let response = ui.button(RichText::new(*title).color(if active {
                                BLUE
                            } else {
                                MUTED
                            }));
                            if response.clicked() {
                                self.settings_page = i;
                            }
                            if active {
                                ui.painter().hline(
                                    response.rect.x_range(),
                                    response.rect.bottom(),
                                    Stroke::new(1.0_f32, BLUE),
                                );
                            }
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_min_width(330.0);
                        match self.settings_page {
                            0 => {
                                ui.heading("Appearance");
                                ui.add_space(8.0);
                                ui.checkbox(&mut self.demo.translucent, "Translucent overlays");
                                ui.separator();
                                ui.label("Note size");
                                ui.add(
                                    Slider::new(&mut self.demo.note_scale, 0.65..=1.6).suffix("x"),
                                );
                                ui.add_space(8.0);
                                ui.label(RichText::new("PALETTE").color(MUTED).size(11.0));
                                ui.horizontal(|ui| {
                                    for color in
                                        [WINDOW, CANVAS, PANEL, CONTROL, BLUE, GOLD, RED, GREEN]
                                    {
                                        let (r, _) = ui
                                            .allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                                        ui.painter().rect_filled(r, 3, color);
                                        ui.painter().rect_stroke(
                                            r,
                                            3,
                                            Stroke::new(1.0_f32, BORDER),
                                            StrokeKind::Inside,
                                        );
                                    }
                                });
                            }
                            1 => {
                                ui.heading("Timeline");
                                ui.add_space(8.0);
                                ui.checkbox(&mut self.demo.show_grid, "Show grid");
                                ui.checkbox(&mut self.demo.snap, "Snap to grid");
                                ui.checkbox(&mut self.demo.show_sensors, "Pad sensor overlay");
                                ui.separator();
                                ui.label("Subdivisions per beat");
                                ui.add(DragValue::new(&mut self.demo.division).range(1..=16));
                            }
                            _ => {
                                ui.heading("Workspace");
                                ui.add_space(8.0);
                                ui.checkbox(&mut self.layout_locked, "Lock layout");
                                ui.separator();
                                if ui.button("Reset current workspace").clicked() {
                                    self.reset_layout();
                                }
                            }
                        }
                    });
                });
            });
        self.settings = open;
    }
}

struct Viewer<'a> {
    demo: &'a mut Demo,
    assets: &'a Assets,
    actions: &'a mut Vec<Action>,
    areas: &'a [(u64, PaneKind)],
    locked: bool,
}

impl TabViewer for Viewer<'_> {
    type Tab = Pane;
    fn title(&mut self, _: &mut Pane) -> WidgetText {
        "".into()
    }
    fn id(&mut self, tab: &mut Pane) -> Id {
        Id::new(("pane", tab.id))
    }
    fn ui(&mut self, ui: &mut Ui, tab: &mut Pane) {
        panels::body(ui, tab, self.demo, self.assets);
    }
    fn scroll_bars(&self, _: &Pane) -> [bool; 2] {
        [false, false]
    }
    fn allowed_in_windows(&self, _: &mut Pane) -> bool {
        false
    }
    fn closeable(&mut self, _: &mut Pane) -> bool {
        false
    }
    fn on_tab_button(&mut self, tab: &mut Pane, response: &Response) {
        area_header(
            tab,
            self.demo,
            response,
            self.actions,
            self.areas,
            self.locked,
            false,
        );
    }
}

fn area_header(
    pane: &mut Pane,
    demo: &mut Demo,
    response: &Response,
    actions: &mut Vec<Action>,
    areas: &[(u64, PaneKind)],
    locked: bool,
    maximized: bool,
) {
    let mut ui = Ui::new(
        response.ctx.clone(),
        response.id.with("area-header"),
        UiBuilder::new()
            .layer_id(response.layer_id)
            .max_rect(response.rect.shrink2(vec2(5.0, 1.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    ui.set_clip_rect(response.rect);
    ui.painter().add(header_surface(response.rect));
    ui.spacing_mut().item_spacing.x = 3.0;
    ui.add_enabled_ui(!locked, |ui| {
        ui.menu_button(glyph(pane.kind.icon(), 13.0).color(MUTED), |ui| {
            for kind in PaneKind::ALL {
                if ui
                    .selectable_label(pane.kind == kind, kind.title())
                    .clicked()
                {
                    actions.push(Action::Replace(pane.id, kind));
                    ui.close_menu();
                }
            }
        })
        .response
        .on_hover_text(format!("{} / Editor type", pane.kind.title()));
    });
    let content_size = vec2((ui.available_width() - 57.0).max(0.0), 24.0);
    ui.allocate_ui_with_layout(content_size, Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_size(content_size);
        ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
        if ui.available_width() >= 155.0 {
            ui.add(Label::new(RichText::new(pane.kind.title()).size(12.0).color(TEXT)).truncate());
            ui.add_space(8.0);
        }
        if ui.available_width() >= 26.0 {
            panels::header_tools(ui, pane, demo);
        }
    });
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.menu_button(glyph(Icon::Ellipsis, 15.0), |ui| {
            area_menu(ui, pane, actions, areas, locked, maximized);
        })
        .response
        .on_hover_text("Area menu");
        if icon(
            ui,
            if maximized {
                Icon::Minimize2
            } else {
                Icon::Maximize2
            },
            if maximized {
                "Restore area"
            } else {
                "Maximize area"
            },
            false,
        )
        .clicked()
        {
            actions.push(if maximized {
                Action::Restore
            } else {
                Action::Maximize(pane.clone())
            });
        }
    });
    if response.clicked() {
        actions.push(Action::Focus(pane.id));
    }
    if response.double_clicked() {
        actions.push(if maximized {
            Action::Restore
        } else {
            Action::Maximize(pane.clone())
        });
    }
    response.context_menu(|ui| area_menu(ui, pane, actions, areas, locked, maximized));
}

fn area_menu(
    ui: &mut Ui,
    pane: &Pane,
    actions: &mut Vec<Action>,
    areas: &[(u64, PaneKind)],
    locked: bool,
    maximized: bool,
) {
    ui.add_enabled_ui(!locked, |ui| {
        if ui.button("Split right").clicked() {
            actions.push(Action::Split(pane.id, Split::Right, pane.kind));
            ui.close_menu();
        }
        if ui.button("Split below").clicked() {
            actions.push(Action::Split(pane.id, Split::Below, pane.kind));
            ui.close_menu();
        }
        ui.add_enabled_ui(areas.len() > 1, |ui| {
            ui.menu_button("Swap with area", |ui| {
                for (index, &(id, kind)) in areas.iter().enumerate() {
                    if id != pane.id
                        && ui
                            .button(format!("{} ({})", kind.title(), index + 1))
                            .clicked()
                    {
                        actions.push(Action::Swap(pane.id, id));
                        ui.close_menu();
                    }
                }
            });
        });
    });
    ui.separator();
    if ui
        .button(if maximized {
            "Restore area"
        } else {
            "Maximize area"
        })
        .clicked()
    {
        actions.push(if maximized {
            Action::Restore
        } else {
            Action::Maximize(pane.clone())
        });
        ui.close_menu();
    }
    if ui
        .add_enabled(!locked && areas.len() > 1, Button::new("Close area"))
        .clicked()
    {
        actions.push(Action::Close(pane.id));
        ui.close_menu();
    }
}
