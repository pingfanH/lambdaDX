//! egui adapter for the standalone Simai input and completion modules.

use crate::{app::types::ChartDoc, state::EditorState, simai_editor::{editing::Selection, history, modal::Style}};
use egui_macroquad::egui::*;

pub struct Prepared {
    before: String,
    selection: Selection,
    changed: bool,
    active: bool,
    previous: Selection,
}

fn from_cursor(range: text::CCursorRange) -> Selection {
    Selection {
        anchor: range.secondary.index,
        head: range.primary.index,
    }
}
fn to_cursor(selection: Selection) -> text::CCursorRange {
    text::CCursorRange {
        primary: text::CCursor::new(selection.head),
        secondary: text::CCursor::new(selection.anchor),
    }
}

pub fn prepare(ui: &mut Ui, editor: &mut EditorState, chart: &mut ChartDoc, id: Id, active: bool, ime: bool) -> Prepared {
    let simai = &mut editor.simai_editor;
    let mut changed = false;
    let previous = simai.input.selection;
    let mut state = text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    let restored = std::mem::take(&mut simai.restore_cursor);
    if restored {
        state.cursor.set_char_range(Some(to_cursor(simai.input.selection)));
        state.clear_undoer();
        state.clone().store(ui.ctx(), id);
    }
    if active {
        if !restored && let Some(range) = state.cursor.char_range() {
            simai.input.selection = from_cursor(range);
        }
        let result = ui.input_mut(|i| {
            history::with_input(editor, chart, |input, text, options, history| {
                input.process_with_history(&mut i.events, text, options, ime, i.time, history)
            })
        });
        changed = result.changed;
        if let Some(text) = result.copied {
            ui.ctx().copy_text(text);
        }
        state
            .cursor
            .set_char_range(Some(to_cursor(editor.simai_editor.input.selection)));
        state.clear_undoer();
        state.store(ui.ctx(), id);
        ui.memory_mut(|m| m.request_focus(id));
    } else {
        editor.simai_editor.input.completion.close();
        editor.simai_editor.history_boundary();
    }
    let simai = &editor.simai_editor;
    Prepared {
        before: simai.text.clone(),
        selection: simai.input.selection,
        changed,
        active,
        previous,
    }
}

pub fn finish(
    ui: &mut Ui,
    editor: &mut EditorState,
    chart: &mut ChartDoc,
    output: &mut text_edit::TextEditOutput,
    prepared: &Prepared,
    enabled: bool,
) -> bool {
    let now = ui.input(|i| i.time);
    if output.response.changed() {
        history::with_input(editor, chart, |input, text, _, history| {
            input.observe_native_with_history(&prepared.before, prepared.selection, text, now, history);
        });
    }
    let simai = &mut editor.simai_editor;
    if let Some(range) = output.cursor_range {
        let selection = from_cursor(range.as_ccursor_range());
        if selection != prepared.selection
            && ui.input(|i| i.pointer.any_down() || i.pointer.any_released())
        {
            simai.input.pointer_selection(selection);
            simai.history_boundary();
        }
        simai.input.selection = selection;
    }
    let active = enabled
        && (output.response.has_focus() || (prepared.active && simai.input.completion.open()));
    if active && (prepared.changed || prepared.previous != simai.input.selection) {
        let cursor = output
            .galley
            .pos_from_ccursor(text::CCursor::new(
                simai.input.cursor(&simai.text, &simai.options),
            ))
            .translate(output.galley_pos.to_vec2());
        if !ui.clip_rect().shrink(8.0).contains_rect(cursor) {
            ui.scroll_to_rect(cursor.expand(8.0), None);
        }
    }
    if active {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                output.response.id,
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
    }
    if active
        && simai.options.edit_style != Style::Standard
        && simai.input.owns_text(&simai.options)
    {
        let at = simai.input.cursor(&simai.text, &simai.options);
        let cursor = output
            .galley
            .pos_from_ccursor(text::CCursor::new(at))
            .translate(output.galley_pos.to_vec2());
        let width = output
            .galley
            .pos_from_ccursor(text::CCursor::new(at + 1))
            .left()
            + output.galley_pos.x
            - cursor.left();
        let rect = Rect::from_min_size(cursor.min, vec2(width.max(7.0), cursor.height()));
        ui.painter().rect_filled(
            rect.intersect(ui.clip_rect()),
            0.0,
            Color32::from_rgba_unmultiplied(120, 195, 235, 100),
        );
    }
    if !active || !simai.input.completion.open() {
        return prepared.changed;
    }
    let cursor = output
        .galley
        .pos_from_ccursor(text::CCursor::new(simai.input.selection.head))
        .translate(output.galley_pos.to_vec2());
    let screen = ui.ctx().screen_rect();
    let height =
        (simai.input.completion.candidates.len().min(7) as f32 * 38.0 + 12.0).min(screen.height());
    let width = 284.0_f32.min(screen.width());
    let x = cursor
        .left()
        .clamp(screen.left(), (screen.right() - width).max(screen.left()));
    let y = if cursor.bottom() + height <= screen.bottom() {
        cursor.bottom() + 2.0
    } else {
        (cursor.top() - height).max(screen.top())
    };
    let mut picked = None;
    Area::new(output.response.id.with("completion"))
        .order(Order::Foreground)
        .fixed_pos(pos2(x, y))
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width((width - 16.0).max(1.0));
                ScrollArea::vertical()
                    .max_height((height - 12.0).max(1.0))
                    .show(ui, |ui| {
                        for (index, candidate) in
                            simai.input.completion.candidates.iter().enumerate()
                        {
                            let label = format!("{}  {}", candidate.label, candidate.detail);
                            let response = ui.add(SelectableLabel::new(
                                index == simai.input.completion.selected,
                                RichText::new(label).monospace(),
                            ));
                            if index == simai.input.completion.selected {
                                response.scroll_to_me(Some(Align::Center));
                            }
                            if response.clicked() {
                                picked = Some(index);
                            }
                        }
                    });
            });
        });
    if let Some(index) = picked {
        simai.input.completion.selected = index;
        history::with_input(editor, chart, |input, text, _, history| {
            input.accept_with_history(text, now, history);
        });
        let simai = &mut editor.simai_editor;
        output
            .state
            .cursor
            .set_char_range(Some(to_cursor(simai.input.selection)));
        output.state.clone().store(ui.ctx(), output.response.id);
        output.cursor_range = output.state.cursor.range(&output.galley);
        output.response.request_focus();
        ui.ctx().request_repaint();
        return true;
    }
    prepared.changed
}
