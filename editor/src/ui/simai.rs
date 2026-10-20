use egui_macroquad::egui::*;
use lucide_icons::Icon;
use crate::app::types::ChartDoc;
use crate::state::EditorState;
use crate::simai_editor::{Options, ClickSeek, NoteHighlight, source::{Kind, tokens}};
use crate::theme::{BG_PANEL, TEXT, TEXT_DIM, ACCENT_BLUE, ACCENT_RED};
use crate::player::state::PadPreviewState;
use super::style::{glyph, icon};
mod editing;

fn code_font() -> FontId { FontId::new(13.0, FontFamily::Name("editor_code".into())) }

#[derive(Default)]
struct Highlighter;

impl cache::ComputerMut<&str, text::LayoutJob> for Highlighter {
    fn compute(&mut self, text: &str) -> text::LayoutJob {
        let mut job = text::LayoutJob::default();
        let mut spans = tokens(text).into_iter().peekable();
        while let Some(mut span) = spans.next() {
            while spans.peek().is_some_and(|next| next.kind == span.kind) {
                span.bytes.end = spans.next().unwrap().bytes.end;
            }
            let color = match span.kind {
                Kind::Bpm => Color32::from_rgb(225, 185, 112),
                Kind::Division => Color32::from_rgb(110, 197, 211),
                Kind::Duration => Color32::from_rgb(149, 199, 145),
                Kind::Speed => Color32::from_rgb(207, 167, 224),
                Kind::Slide => ACCENT_BLUE,
                Kind::Modifier => Color32::from_rgb(232, 153, 167),
                Kind::Separator | Kind::Comment => TEXT_DIM,
                Kind::Note | Kind::Space => TEXT,
            };
            job.append(&text[span.bytes], 0.0, TextFormat { font_id: code_font(), color, ..Default::default() });
        }
        job
    }
}

pub fn options(ui: &mut Ui, options: &mut Options) {
    ui.label("编辑模式");
    ui.horizontal(|ui| {
        use crate::simai_editor::modal::Style;
        ui.selectable_value(&mut options.edit_style, Style::Standard, "标准");
        ui.selectable_value(&mut options.edit_style, Style::Vim, "Vim");
        ui.selectable_value(&mut options.edit_style, Style::Helix, "Helix");
    });
    ui.checkbox(&mut options.completion, "代码补全");
    ui.add_enabled_ui(options.completion, |ui| { ui.checkbox(&mut options.auto_completion, "自动弹出候选"); });
    ui.checkbox(&mut options.auto_pairs, "括号自动配对");
    ui.separator();
    ui.checkbox(&mut options.highlight, "语法高亮");
    ui.checkbox(&mut options.follow, "代码跟随播放");
    ui.checkbox(&mut options.follow_cursor, "时间轴跟随文本光标")
        .on_hover_text("Alt+Shift+Enter 切换；Alt+Enter 单次跳转到光标");
    ui.checkbox(&mut options.space_playback, "空格用于播放/暂停")
        .on_hover_text("仅在 Simai 分区生效；启用后按空格不再输入空格，粘贴不受影响");
    ui.checkbox(&mut options.live_apply, "实时应用文本");
    ui.separator();
    ui.label("点击跳转");
    ui.radio_value(&mut options.click_seek, ClickSeek::Off, "关闭");
    ui.radio_value(&mut options.click_seek, ClickSeek::Click, "直接点击");
    ui.radio_value(&mut options.click_seek, ClickSeek::CommandClick, "Command / Ctrl + 点击");
    ui.separator();
    ui.label("Note 背景高亮");
    ui.radio_value(&mut options.note_highlight, NoteHighlight::Off, "关闭高亮");
    ui.radio_value(&mut options.note_highlight, NoteHighlight::Visible, "所有出现的 Note");
    ui.radio_value(&mut options.note_highlight, NoteHighlight::Current, "仅当前拍点");
}

pub fn header(ui: &mut Ui, editor: &mut EditorState, chart: &mut ChartDoc) {
    ui.add_enabled_ui(editor.template_session.is_none(), |ui| {
        let modified = editor.simai_editor.modified();
        ui.add_enabled_ui(modified && !editor.simai_editor.conflict(chart), |ui| {
            if icon(ui, Icon::Check, "应用文本 (Cmd/Ctrl+Enter)", false).clicked() {
                let _ = crate::simai_editor::apply(editor, chart);
            }
        });
        if icon(ui, Icon::RotateCcw, "放弃文本草稿，从谱面重新读取", false).clicked() {
            editor.simai_editor.refresh(chart);
        }
        if icon(ui, Icon::Save, "应用并保存谱面 (Cmd/Ctrl+S)", false).clicked() {
            crate::io::save_document(editor, chart);
        }
        if icon(ui, Icon::MoveRight, "代码跟随播放", editor.simai_editor.options.follow).clicked() {
            editor.simai_editor.options.follow = !editor.simai_editor.options.follow;
        }
        ui.menu_button(glyph(Icon::SlidersHorizontal, 14.0), |ui| options(ui, &mut editor.simai_editor.options))
            .response.on_hover_text("Simai 编辑设置");
        if ui.available_width() > 100.0 {
            let mode = editor.simai_editor.input.modal.label(editor.simai_editor.options.edit_style);
            if !mode.is_empty() { ui.add(Label::new(RichText::new(mode).monospace().color(ACCENT_BLUE)).truncate()); }
            ui.label(RichText::new(if modified { "未应用" } else { "主谱面" })
                .color(if modified { ACCENT_BLUE } else { TEXT_DIM }));
        }
    });
}

pub fn draw(ui: &mut Ui, editor: &mut EditorState, app: &mut PadPreviewState, now: f32, enabled: bool, focused: bool) {
    let speed = if crate::app::params::speed_scales_visuals() { app.play_speed.max(0.05) } else { 1.0 };
    let visible: std::collections::HashSet<_> = if editor.simai_editor.options.note_highlight == NoteHighlight::Visible {
        app.chart.notes.iter().filter(|note| {
            let timing = crate::player::render::timing::compute(note, app, &app.chart.bpms, now, speed);
            crate::player::render::notes::is_visible(app, note, &timing)
        }).map(|note| note.id).collect()
    } else { Default::default() };
    let chart = &mut app.chart;
    let template = editor.template_session.is_some();
    if !template { editor.simai_editor.sync(chart); }
    let text_id = Id::new(("simai-text", editor.simai_editor.generation));
    let owns_keyboard = ui.memory(|m| m.has_focus(text_id)) || (focused && !ui.ctx().wants_keyboard_input());
    let jump_cursor = enabled && !template && owns_keyboard && !editor.keyboard.ime
        && ui.input_mut(|i| consume_cursor_shortcuts(&mut i.events, &mut editor.simai_editor.options.follow_cursor));
    if enabled && !template && editor.simai_editor.options.space_playback
        && editor.simai_editor.input.modal.search.is_none()
        && (editor.simai_editor.options.edit_style == crate::simai_editor::modal::Style::Standard
            || !editor.simai_editor.input.inserting(&editor.simai_editor.options))
        && owns_keyboard
        && ui.input_mut(|i| consume_play_space(&mut i.events, editor.keyboard.ime)) {
        editor.keyboard.transport.push(crate::keyboard::Transport::Toggle);
    }
    if enabled && focused && !template {
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
            let _ = crate::simai_editor::apply(editor, chart);
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)) {
            editor.save_as_request = true;
        } else if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            crate::io::save_document(editor, chart);
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::E)) {
            editor.export_request = true;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            editor.open_dialog = true;
        }
    }
    let prepared = editing::prepare(ui, editor, chart, text_id,
        enabled && !template && owns_keyboard, editor.keyboard.ime);
    let active = if !template && !editor.simai_editor.modified() {
        let simai = &mut editor.simai_editor;
        simai.playback.update(&simai.text, chart);
        simai.playback.active(now).cloned()
    } else { None };
    let follow_id = ui.id().with("simai-follow-position");
    let target = active.as_ref().filter(|_| editor.simai_editor.options.follow).map(|cell| cell.chars.start);
    let follow_changed = ui.data(|d| d.get_temp::<Option<usize>>(follow_id)).flatten() != target;
    ui.data_mut(|d| d.insert_temp(follow_id, target));
    let mut seek_time = None;
    Frame::NONE.fill(crate::theme::surface(BG_PANEL)).inner_margin(8).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        if template { ui.colored_label(TEXT_DIM, "模板编辑中"); }
        if editor.simai_editor.conflict(chart) {
            ui.colored_label(Color32::from_rgb(224, 177, 99), "主谱面和 Simai 草稿都有修改；草稿已保留，请备份后重新读取谱面。");
        }
        if let Some(error) = &editor.simai_editor.error {
            ScrollArea::vertical().id_salt("simai-errors").max_height(72.0).show(ui, |ui| {
                ui.colored_label(Color32::from_rgb(235, 126, 138), error);
            });
        }
        if let Some(error) = &editor.simai_editor.playback.error {
            ui.colored_label(TEXT_DIM, error);
        }
        crate::ui::style::enabled_ui(ui, enabled && !template, |ui| {
            let height = ui.available_height();
            ScrollArea::both().id_salt("simai-text-scroll").auto_shrink([false, false]).show(ui, |ui| {
                let width = ui.available_width();
                let rows = editor.simai_editor.text.split('\n').count();
                let font = code_font();
                let line_height = ui.fonts(|f| f.row_height(&font));
                ui.horizontal_top(|ui| {
                    let gutter = 12.0 + (rows.max(10).ilog10() + 1) as f32 * 8.0;
                    let (rect, _) = ui.allocate_exact_size(vec2(gutter, (rows as f32 * line_height).max(1.0)), Sense::hover());
                    let start = ((ui.clip_rect().top() - rect.top()) / line_height).floor().max(0.0) as usize;
                    let end = (((ui.clip_rect().bottom() - rect.top()) / line_height).ceil() as usize + 1).min(rows);
                    for row in start..end {
                        ui.painter().text(pos2(rect.right() - 6.0, rect.top() + row as f32 * line_height),
                            Align2::RIGHT_TOP, (row + 1).to_string(), font.clone(), TEXT_DIM);
                    }
                    let background = ui.painter().add(Shape::Noop);
                    let mut layouter = |ui: &Ui, text: &str, _: f32| {
                        let job = ui.memory_mut(|m| m.caches.cache::<cache::FrameCache<text::LayoutJob, Highlighter>>().get(text));
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let mut edit = TextEdit::multiline(&mut editor.simai_editor.text)
                        .id(text_id)
                        .code_editor().font(font).text_color(TEXT).frame(false).margin(Vec2::ZERO)
                        .desired_width(f32::INFINITY).min_size(vec2((width - gutter - 8.0).max(40.0), height));
                    if editor.simai_editor.options.highlight { edit = edit.layouter(&mut layouter); }
                    let cursor_style = ui.visuals().text_cursor.clone();
                    if editor.simai_editor.input.owns_text(&editor.simai_editor.options) {
                        ui.visuals_mut().text_cursor.stroke = Stroke::NONE;
                        ui.visuals_mut().text_cursor.preview = false;
                    }
                    let mut output = edit.show(ui);
                    ui.visuals_mut().text_cursor = cursor_style;
                    let edited = editing::finish(ui, editor, chart, &mut output, &prepared, enabled && !template);
                    if output.response.changed() || edited {
                        editor.simai_editor.changed();
                        ui.data_mut(|d| d.insert_temp::<Option<usize>>(follow_id, None));
                    }
                    if !template {
                        let mut shapes = Vec::new();
                        let mut offset = 0;
                        let row_starts: Vec<_> = output.galley.rows.iter().map(|row| {
                            let start = offset;
                            offset += row.char_count_including_newline();
                            start
                        }).collect();
                        for range in editor.simai_editor.input.modal.search_ranges(&editor.simai_editor.text) {
                            range_background(&output, &row_starts, &range, ui.clip_rect(),
                                Color32::from_rgba_unmultiplied(210, 170, 50, 90), &mut shapes);
                        }
                        let map = &editor.simai_editor.playback;
                        let mapped = !editor.simai_editor.modified();
                        for cell in map.cells.iter().filter(|_| mapped) {
                            if editor.loop_enabled && editor.loop_range.is_some_and(|(a, b)| cell.time >= a && cell.time < b) {
                                for range in &cell.code { range_background(&output, &row_starts, range, ui.clip_rect(), loop_color(), &mut shapes); }
                            }
                        }
                        for cell in map.cells.iter().filter(|_| mapped) {
                            for note in &cell.notes {
                                let highlight = match editor.simai_editor.options.note_highlight {
                                    NoteHighlight::Off => false,
                                    NoteHighlight::Visible => note.ids.iter().any(|id| visible.contains(id)),
                                    NoteHighlight::Current => active.as_ref().is_some_and(|active| active.chars == cell.chars),
                                };
                                if highlight {
                                    for range in &note.chars { range_background(&output, &row_starts, range, ui.clip_rect(),
                                        Color32::from_rgba_unmultiplied(65, 139, 220, 74), &mut shapes); }
                                }
                            }
                        }
                        ui.painter().set(background, Shape::Vec(shapes));
                        let seek = match editor.simai_editor.options.click_seek {
                            ClickSeek::Off => false,
                            ClickSeek::Click => ui.input(|i| !i.modifiers.shift && !i.modifiers.alt),
                            ClickSeek::CommandClick => ui.input(|i| i.modifiers.command && !i.modifiers.shift && !i.modifiers.alt),
                        };
                        if mapped && enabled && seek && output.response.clicked()
                            && let Some(pos) = output.response.interact_pointer_pos()
                            && let Some(index) = hit_character(&output, pos)
                            && let Some(cell) = map.at_char(index) {
                            seek_time = Some(cell.time);
                        }
                        if let Some(cell) = active.as_ref().filter(|_| mapped) {
                        let cursor = output.galley.pos_from_ccursor(text::CCursor::new(cell.chars.start))
                            .translate(output.galley_pos.to_vec2());
                        ui.painter().vline(cursor.left(), cursor.y_range(), Stroke::new(2.0_f32, ACCENT_RED));
                        ui.painter().hline((cursor.left() - 3.0)..=(cursor.left() + 3.0), cursor.top(), Stroke::new(2.0_f32, ACCENT_RED));
                        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.right() - 2.0, cursor.top()),
                            pos2(rect.right(), cursor.bottom())), 0.0, ACCENT_RED);
                        if enabled && follow_changed && target.is_some()
                            && !ui.clip_rect().shrink2(vec2(16.0, line_height)).contains_rect(cursor) {
                            ui.scroll_to_rect(cursor.expand2(vec2(24.0, line_height)), Some(Align::Center));
                        }
                        }
                    }
                    if enabled && !template && !editor.keyboard.ime && (owns_keyboard || output.response.has_focus())
                        && output.cursor_range.is_some()
                        && let Some(time) = {
                            let simai = &mut editor.simai_editor;
                            let index = simai.input.cursor(&simai.text, &simai.options);
                            simai.cursor_seek(index, jump_cursor)
                        } {
                        seek_time = Some(time);
                    }
                });
            });
        });
    });
    if let Some(time) = seek_time {
        editor.keyboard.transport.push(crate::keyboard::Transport::Seek(time));
        editor.view_start = time;
        editor.keyboard.cursor.measure = crate::app::types::secs_to_measure(time, &chart.bpms);
    }
}

fn consume_cursor_shortcuts(events: &mut Vec<Event>, follow: &mut bool) -> bool {
    if events.iter().any(|e| matches!(e, Event::Ime(event) if !matches!(event, ImeEvent::Disabled))) {
        return false;
    }
    let mut jump = false;
    events.retain(|event| {
        if let Event::Key { key, modifiers, pressed, repeat, .. } = event
            && let Some(command) = crate::keyboard::cursor_command(*key, *modifiers) {
            if *pressed && !repeat {
                match command {
                    crate::keyboard::CursorCommand::Jump => jump = true,
                    crate::keyboard::CursorCommand::ToggleFollow => *follow = !*follow,
                }
            }
            return false;
        }
        true
    });
    jump
}

fn loop_color() -> Color32 { crate::theme::loop_fill(true) }

fn range_background(output: &text_edit::TextEditOutput, row_starts: &[usize], range: &std::ops::Range<usize>, clip: Rect, color: Color32, shapes: &mut Vec<Shape>) {
    let first = row_starts.partition_point(|&start| start <= range.start).saturating_sub(1);
    for (row, &offset) in output.galley.rows.iter().zip(row_starts).skip(first) {
        if offset >= range.end || row.min_y() + output.galley_pos.y > clip.bottom() { break; }
        if row.max_y() + output.galley_pos.y < clip.top() { continue; }
        let end = offset + row.char_count_excluding_newline();
        if range.start < end && range.end > offset {
            let rect = Rect::from_min_max(
                pos2(row.x_offset(range.start.saturating_sub(offset)), row.min_y()),
                pos2(row.x_offset(range.end.min(end) - offset), row.max_y()))
                .translate(output.galley_pos.to_vec2());
            if rect.intersects(clip) { shapes.push(Shape::rect_filled(rect, 1.0, color)); }
        }
    }
}

fn hit_character(output: &text_edit::TextEditOutput, pos: Pos2) -> Option<usize> {
    let local = pos - output.galley_pos;
    let mut offset = 0;
    for row in &output.galley.rows {
        if local.y >= row.min_y() && local.y <= row.max_y() {
            return row.glyphs.iter().enumerate().find(|(_, glyph)|
                local.x >= glyph.pos.x && local.x < glyph.pos.x + glyph.advance_width)
                .map(|(index, _)| offset + index);
        }
        offset += row.char_count_including_newline();
    }
    None
}

fn consume_play_space(events: &mut Vec<Event>, ime: bool) -> bool {
    if ime || events.iter().any(|e| matches!(e, Event::Ime(event) if !matches!(event, ImeEvent::Disabled))) {
        return false;
    }
    let mut toggle = false;
    for event in events.iter() {
        if let Event::Key { key: Key::Space, pressed: true, repeat, modifiers, .. } = event
            && modifiers.is_none() {
            toggle |= !repeat;
        }
    }
    events.retain(|event| match event {
        Event::Key { key: Key::Space, modifiers, .. } if modifiers.is_none() => false,
        // Native character input can arrive a frame before or after key input.
        Event::Text(text) if text == " " => false,
        _ => true,
    });
    toggle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_space_consumes_typing_but_not_paste_repeat_or_ime() {
        let key = |repeat| Event::Key { key: Key::Space, physical_key: None, pressed: true, repeat, modifiers: Modifiers::NONE };
        let mut events = vec![key(false), Event::Text(" ".into()), Event::Paste("1, 2,".into())];
        assert!(consume_play_space(&mut events, false));
        assert!(matches!(&events[..], [Event::Paste(_)]));
        let mut events = vec![key(true), Event::Text(" ".into())];
        assert!(!consume_play_space(&mut events, false));
        assert!(events.is_empty());
        let mut events = vec![key(false), Event::Text(" ".into())];
        assert!(!consume_play_space(&mut events, true));
        assert_eq!(events.len(), 2);
        events.push(Event::Ime(ImeEvent::Commit("词".into())));
        assert!(!consume_play_space(&mut events, false));
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn playback_space_filters_separate_character_frames_without_toggling() {
        let mut events = vec![Event::Text(" ".into()), Event::Text("1".into()), Event::Paste(" ".into())];
        assert!(!consume_play_space(&mut events, false));
        assert_eq!(events, vec![Event::Text("1".into()), Event::Paste(" ".into())]);
        let mut events = vec![Event::Key { key: Key::Space, physical_key: None,
            pressed: true, repeat: false, modifiers: Modifiers::NONE }];
        assert!(consume_play_space(&mut events, false));
        assert!(events.is_empty());
        let mut events = vec![Event::Text(" ".into())];
        assert!(!consume_play_space(&mut events, false));
        assert!(events.is_empty());
    }
}
