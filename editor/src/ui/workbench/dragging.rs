use super::*;

impl Workbench {
    pub(super) fn finish_drag(&mut self, ctx: &Context, regions: &[Region], enabled: bool, actions: &mut Vec<Action>) {
        let Some(source) = self.dragging else { return; };
        self.drag_owned = true;
        if !enabled || self.locked || self.maximized.is_some()
            || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
            || ctx.input(|i| i.pointer.secondary_clicked()) {
            self.dragging = None;
            return;
        }
        ctx.set_cursor_icon(CursorIcon::Grabbing);
        let target = ctx.input(|i| i.pointer.interact_pos()).and_then(|pos| {
            regions.iter().find_map(|region| {
                let rect = Rect::from_min_max(region.rect.min - vec2(0.0, 26.0), region.rect.max);
                (region.pane_id != source && rect.contains(pos))
                    .then(|| (region.pane_id, drop_target(rect, pos)))
            })
        });
        if let Some((target, (direction, preview))) = target {
            let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("pane-drop-preview")));
            painter.rect_filled(preview.shrink(2.0), 2, ACCENT_BLUE.gamma_multiply(0.17));
            painter.rect_stroke(preview.shrink(2.0), 2, Stroke::new(1.5_f32, ACCENT_BLUE), StrokeKind::Inside);
            if ctx.input(|i| i.pointer.button_released(PointerButton::Primary)) {
                actions.push(match direction {
                    Some(direction) => Action::Move(source, target, direction),
                    None => Action::Swap(source, target),
                });
            }
        }
        if !ctx.input(|i| i.pointer.primary_down()) { self.dragging = None; }
    }
}

fn drop_target(rect: Rect, pos: Pos2) -> (Option<Split>, Rect) {
    let x = (pos.x - rect.left()) / rect.width().max(1.0);
    let y = (pos.y - rect.top()) / rect.height().max(1.0);
    let (distance, side) = [(x, Split::Left), (1.0 - x, Split::Right), (y, Split::Above), (1.0 - y, Split::Below)]
        .into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
    if distance >= 0.25 { return (None, rect); }
    let mut preview = rect;
    match side {
        Split::Left => preview.max.x = rect.center().x,
        Split::Right => preview.min.x = rect.center().x,
        Split::Above => preview.max.y = rect.center().y,
        Split::Below => preview.min.y = rect.center().y,
    }
    (Some(side), preview)
}
