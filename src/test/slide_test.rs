use lambda_dx::app::pad_svg::{PadSvgDef, draw_polygon_fill, draw_polygon_lines};
use lambda_dx::app::slide_render::{self, SlideTextures};
use lambda_dx::app::types::zone::PadZone;
use lambda_dx::app::types::{
    Note, NoteType, PAD_C_ZONE, PAD_ROTATION_RAD, PadGeom, Slide, SlidePoint, SlideSegment,
    SlideShape, TAP_RING_OFFSET, note_secs, secs_to_measure,
};
use macroquad::prelude::*;

// ── Render mode ──
#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Show,
    Live,
}

const DEFAULT_MODE: RenderMode = RenderMode::Show;

// ── Parsed slide (note + sub-slide, with pre-computed seconds) ──
struct ParsedSlide {
    note: Note,
    slide: Slide,
    dur_s: f32,
    delay_s: f32,
}

#[macroquad::main("Slide Test")]
async fn main() {
    set_pc_assets_folder("assets");

    let mut input_text = String::from("3z7");
    let mut bpm: f32 = 180.0;
    let mut parsed: Vec<ParsedSlide> = vec![];
    let mut msg = String::new();
    let mut current_t: f32 = -0.5;
    let mut playing = false;
    let mut speed: f32 = 1.0;
    let mut mode: RenderMode = DEFAULT_MODE;

    // Load SVG zones from the library
    let pad_svg = PadSvgDef::from_svg_str(include_str!("../../assets/pad.svg")).ok();

    let slide_tex = load_texture("Skins/classic/slide.png").await.ok();
    let star_tex = load_texture("Skins/classic/star.png").await.ok();

    // Parse initial
    let mut prev_input = input_text.clone();
    let mut prev_bpm = bpm;
    parse(&input_text, bpm, &mut parsed, &mut msg);

    loop {
        clear_background(Color::from_rgba(10, 17, 30, 255));
        let sw = screen_width();
        let sh = screen_height();

        // ── Layout ──
        let side = sw.min(sh) * 0.85;
        let cx = sw * 0.5;
        let cy = sh * 0.48;
        let outer_r = side * 0.42;
        let scale = outer_r / 326.57;

        let pad_geom = PadGeom { cx, cy, outer_r };

        // Spawn center from C zone
        let (spawn_cx, spawn_cy) = pad_svg
            .as_ref()
            .and_then(|svg| svg.pad_visual_center(&pad_geom))
            .map(|v| (v.x, v.y))
            .unwrap_or((cx, cy));

        // ── Tick (live mode only) ──
        if mode == RenderMode::Live && playing {
            current_t += get_frame_time() * speed;
            let max_t = parsed
                .iter()
                .map(|s| note_secs(&s.note, &[]) + s.dur_s)
                .fold(0.0_f32, f32::max);
            if current_t > max_t + 2.0 {
                current_t = -1.0;
            }
        }

        // ── Draw bg zones ──
        draw_pad_background(&pad_geom, pad_svg.as_ref(), scale, spawn_cx, spawn_cy);

        // ── Draw slides ──
        if let Some(ref svg) = pad_svg {
            let show_full = mode == RenderMode::Show;
            let tex = SlideTextures {
                trail: slide_tex.as_ref(),
                star: star_tex.as_ref(),
                star_fallback: star_tex.as_ref(),
                star_ex: None,
                star_ex_fallback: None,
                wifi: [None; 11],
            };

            for ps in &parsed {
                let ns = note_secs(&ps.note, &[]);
                slide_render::draw_slide(
                    &ps.note,
                    &ps.slide,
                    current_t,
                    ns,
                    ps.dur_s,
                    ps.delay_s,
                    &pad_geom,
                    svg,
                    scale,
                    vec2(spawn_cx, spawn_cy),
                    outer_r,
                    &tex,
                    show_full,
                    1.0,
                    7.5,
                    3.926_913 / 7.5,
                    0,
                );
            }
        }

        // ── Egui UI ──
        egui_macroquad::ui(|ctx| {
            egui_macroquad::egui::TopBottomPanel::top("bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let mode_label = match mode {
                        RenderMode::Show => "SHOW",
                        RenderMode::Live => "LIVE",
                    };
                    let mode_color = match mode {
                        RenderMode::Show => egui_macroquad::egui::Color32::from_rgb(255, 220, 50),
                        RenderMode::Live => egui_macroquad::egui::Color32::from_rgb(103, 232, 249),
                    };
                    if ui
                        .add_sized(
                            [60.0, 22.0],
                            egui_macroquad::egui::Button::new(
                                egui_macroquad::egui::RichText::new(mode_label).color(mode_color),
                            ),
                        )
                        .clicked()
                    {
                        mode = match mode {
                            RenderMode::Show => RenderMode::Live,
                            RenderMode::Live => RenderMode::Show,
                        };
                        playing = false;
                        current_t = -0.5;
                    }
                    ui.separator();
                    ui.label("Slide:");
                    ui.add(
                        egui_macroquad::egui::TextEdit::singleline(&mut input_text)
                            .desired_width(360.0),
                    );
                    if ui.button("Parse").clicked() {
                        parse(&input_text, bpm, &mut parsed, &mut msg);
                    }
                    ui.label("BPM:");
                    ui.add(
                        egui_macroquad::egui::DragValue::new(&mut bpm)
                            .speed(5)
                            .range(30..=999),
                    );
                    if mode == RenderMode::Live {
                        if ui.button(if playing { "⏸" } else { "▶" }).clicked() {
                            playing = !playing;
                            if playing && current_t < 0.0 {
                                current_t = -0.5;
                            }
                        }
                        ui.label("Speed:");
                        ui.add(
                            egui_macroquad::egui::DragValue::new(&mut speed)
                                .speed(0.1)
                                .range(0.1..=3.0),
                        );
                        if ui.button("Reset").clicked() {
                            current_t = -0.5;
                        }
                    }
                    ui.label(&msg);
                });
            });
        });
        egui_macroquad::draw();

        // Auto-parse on change
        if input_text != prev_input || bpm != prev_bpm {
            parse(&input_text, bpm, &mut parsed, &mut msg);
            prev_input = input_text.clone();
            prev_bpm = bpm;
        }

        if is_key_pressed(KeyCode::Enter) {
            parse(&input_text, bpm, &mut parsed, &mut msg);
        }
        if is_key_pressed(KeyCode::Space) {
            playing = !playing;
            if playing && current_t < 0.0 {
                current_t = -0.5;
            }
        }
        if is_key_pressed(KeyCode::R) {
            current_t = -0.5;
        }

        next_frame().await;
    }
}

// ── Pad background (zones + A-ring) ──

fn draw_pad_background(
    pad: &PadGeom,
    pad_svg: Option<&PadSvgDef>,
    scale: f32,
    spawn_cx: f32,
    spawn_cy: f32,
) {
    draw_circle(
        pad.cx,
        pad.cy,
        pad.outer_r,
        Color::from_rgba(16, 24, 38, 255),
    );

    // SVG zones
    if let Some(svg) = pad_svg {
        for def in &svg.zones {
            let verts = svg.def_screen_verts(def, pad);
            let centroid = svg.def_screen_centroid(def, pad);
            draw_polygon_fill(&verts, Color::from_rgba(30, 41, 59, 255));
            draw_polygon_lines(&verts, 2.0 * scale, Color::from_rgba(71, 85, 105, 255));
            // Zone label
            let ts = 17.0 * scale;
            let dims = measure_text(&def.label, None, ts as _, 1.0);
            draw_text(
                &def.label,
                centroid.x - dims.width * 0.5,
                centroid.y + dims.height * 0.35,
                ts,
                Color::from_rgba(148, 163, 184, 255),
            );
        }
    }
    let spawn_cx = pad_svg
        .unwrap()
        .pad_visual_center(pad)
        .unwrap_or(vec2(pad.cx, pad.cy));
    // A-zone octagon ring
    // Draw A-zone tap indicators with connecting octagon
    // Draw A-zone tap indicators as a perfect circle centered on spawn_cx
    let dot_r = pad.outer_r + TAP_RING_OFFSET * scale;
    let mut a_dots: Vec<Vec2> = Vec::new();
    for i in 0..8 {
        let ang = -std::f32::consts::FRAC_PI_2
            + PAD_ROTATION_RAD
            + i as f32 * std::f32::consts::TAU / 8.0;
        a_dots.push(vec2(
            spawn_cx.x + ang.cos() * dot_r,
            spawn_cx.y + ang.sin() * dot_r,
        ));
    }
    // 圆弧连接 8 个 tap 圆点

    let arc_steps = 8;
    for i in 0..8 {
        let a0 = -std::f32::consts::FRAC_PI_2
            + PAD_ROTATION_RAD
            + i as f32 * std::f32::consts::TAU / 8.0;
        let a1 = -std::f32::consts::FRAC_PI_2
            + PAD_ROTATION_RAD
            + (i + 1) as f32 * std::f32::consts::TAU / 8.0;
        for j in 0..arc_steps {
            let t0 = j as f32 / arc_steps as f32;
            let t1 = (j + 1) as f32 / arc_steps as f32;
            let ang0 = a0 + (a1 - a0) * t0;
            let ang1 = a0 + (a1 - a0) * t1;
            draw_line(
                spawn_cx.x + ang0.cos() * dot_r,
                spawn_cx.y + ang0.sin() * dot_r,
                spawn_cx.x + ang1.cos() * dot_r,
                spawn_cx.y + ang1.sin() * dot_r,
                2.0 * scale,
                Color::from_rgba(255, 255, 255, 120),
            );
        }
    }
    for dot in &a_dots {
        draw_circle(
            dot.x,
            dot.y,
            5.0 * scale,
            Color::from_rgba(255, 255, 255, 220),
        );
    }
    draw_circle(
        spawn_cx.x,
        spawn_cx.y,
        3.0 * scale,
        Color::from_rgba(255, 255, 255, 180),
    );
}

// ── Simai parser ──

fn parse(input: &str, bpm: f32, out: &mut Vec<ParsedSlide>, msg: &mut String) {
    out.clear();
    let beat_s = 60.0 / bpm;
    let mut t: f32 = 0.0;
    let mut div: f32 = 4.0;
    let mut n: usize = 0;

    for part in input.split(',') {
        let p = part.trim();
        if p.is_empty() {
            t += beat_s * 4.0 / div;
            continue;
        }
        if p.starts_with('{') && p.ends_with('}') {
            if let Ok(d) = p[1..p.len() - 1].parse::<f32>() {
                div = d;
            }
            continue;
        }
        if let Some(ps) = parse_one_slide(p, t, beat_s) {
            out.push(ps);
            n += 1;
        }
        t += beat_s * 4.0 / div;
    }
    *msg = format!("{} slides", n);
}

fn parse_one_slide(s: &str, time_s: f32, beat_s: f32) -> Option<ParsedSlide> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let start_lane = chars[0].to_digit(10)? as u8;
    if start_lane < 1 || start_lane > 8 {
        return None;
    }
    if chars.len() < 2 {
        return None;
    }

    let rest: String = chars[1..].iter().collect();
    let bracket_pos = rest.find('[');
    let mid_part = if let Some(pos) = bracket_pos {
        &rest[..pos]
    } else {
        &rest
    };
    let bracket_part = if let Some(pos) = bracket_pos {
        &rest[pos..]
    } else {
        ""
    };

    // Parse duration/delay from [X:Y] or [X#Y] — Simai units: 1 = 0.01 beat
    let (dur_s, delay_s) = if !bracket_part.is_empty() {
        let inner = bracket_part.trim_matches(|c| c == '[' || c == ']');
        let (dur_u, delay_u) = if let Some(h) = inner.find('#') {
            (
                inner[..h].parse::<f32>().unwrap_or(8.0),
                inner[h + 1..].parse::<f32>().unwrap_or(1.0),
            )
        } else if let Some(c) = inner.find(':') {
            (
                inner[..c].parse::<f32>().unwrap_or(8.0),
                inner[c + 1..].parse::<f32>().unwrap_or(1.0),
            )
        } else {
            (inner.parse::<f32>().unwrap_or(8.0), 1.0)
        };
        (dur_u * 0.01 * beat_s * 4.0, delay_u * 0.01 * beat_s * 4.0)
    } else {
        (0.3, 0.125 * beat_s)
    };

    // Detect shape
    let shape = {
        let s_lower: String = mid_part
            .chars()
            .filter(|c| !c.is_ascii_digit() && *c != '-')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        match s_lower.as_str() {
            "pp" => SlideShape::PP,
            "qq" => SlideShape::QQ,
            "p" => SlideShape::P,
            "q" => SlideShape::Q,
            "s" => SlideShape::S,
            "z" => SlideShape::Z,
            "w" => SlideShape::Wifi,
            "v" => SlideShape::VShape,
            ">" => SlideShape::Right,
            "<" => SlideShape::Left,
            "^" => SlideShape::Caret,
            "bv" | "vb" => SlideShape::BigV,
            "" => SlideShape::Line,
            _ => SlideShape::Line,
        }
    };

    // Extract digits
    let digits: Vec<u8> = mid_part
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| d as u8)
        .filter(|&d| d >= 1 && d <= 8)
        .collect();
    let dedup_digits: Vec<u8> = digits.iter().fold(vec![], |mut acc, &d| {
        if acc.last() != Some(&d) {
            acc.push(d);
        }
        acc
    });

    if dedup_digits.is_empty() {
        return None;
    }

    let end_lane = *dedup_digits.last().unwrap();
    let mid_zones: Vec<u8> = dedup_digits[..dedup_digits.len() - 1].to_vec();

    // For V-shape without explicit mid zone, insert center zone 17
    let mid_zones =
        if matches!(shape, SlideShape::VShape | SlideShape::BigV) && mid_zones.is_empty() {
            vec![PAD_C_ZONE]
        } else {
            mid_zones
        };

    // Build zone list: start → mid_zones → end
    let mut zone_order = vec![start_lane];
    zone_order.extend(&mid_zones);
    zone_order.push(end_lane);

    let points: Vec<SlidePoint> = zone_order
        .iter()
        .skip(1) // skip start lane
        .map(|&z| SlidePoint {
            zone: PadZone::from(z),
            beat_offset: 0.0,
        })
        .collect();

    // Convert seconds to measures for the Note/Slide types. `dur_s` is the
    // total span from the head (travel + delay), matching the runtime model.
    let time_measure = secs_to_measure(time_s, &[]);
    let delay_measure = secs_to_measure(time_s + delay_s, &[]) - time_measure;
    let dur_s = (dur_s + delay_s).max(0.1);
    let delay_s = delay_s.max(0.0).min(dur_s - 0.001);
    let slide_dur_measure = secs_to_measure(time_s + dur_s, &[]) - time_measure;

    let note = Note {
        time: time_measure,
        lane: start_lane,
        note_type: NoteType::Slide,
        slide: vec![],
        ..Default::default()
    };
    let slide = Slide {
        segments: vec![SlideSegment { points, shape }],
        slide_duration: slide_dur_measure,
        slide_start_delay: delay_measure,
        slide_is_break: false,
    };

    Some(ParsedSlide {
        note,
        slide,
        dur_s,
        delay_s,
    })
}
