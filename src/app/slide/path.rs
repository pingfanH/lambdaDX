use macroquad::math::{vec2, Vec2};
use crate::app::pad_svg::PadSvgDef;
use crate::app::types::{Note, PadGeom, SlideSegment, SlideShape, PAD_ROTATION_RAD, SLIDE_TILE_SPACING, TAP_TARGET_OFFSET};
use crate::app::types::zone::PadZone;
pub trait SlidePath{
    fn path(mut path:Vec<Vec2>,note: &Note,seg:SlideSegment,outer_r:f32,spawn_cx:Vec2,pad: &PadGeom,
            svg: &PadSvgDef,scale: f32,){}
}
impl SlidePath for SlideShape::Q{
    fn path(mut path:Vec<Vec2>,note: &Note,seg:SlideSegment,outer_r:f32,spawn_cx:Vec2, pad: &PadGeom,
            svg: &PadSvgDef,scale: f32) {
        let get_zone_pos=|zone: PadZone|{
            let idx = (zone.to_id() - 1) as f32;
            let ang = -std::f32::consts::FRAC_PI_2 + PAD_ROTATION_RAD + idx * std::f32::consts::TAU / 8.0;
            let target_r = outer_r + TAP_TARGET_OFFSET;
            vec2(spawn_cx.x + ang.cos() * target_r, spawn_cx.y + ang.sin() * target_r)
        };
        let sp = if seg.points.len() == 1 {
            seg.points.first().unwrap()
        } else {
           return;
        };

        // ── 终点坐标：A 区用八边形环上位置，其他区域取 centroid ──
        let end = if sp.zone.to_id() <= 8 {
            get_zone_pos(sp.zone)
        } else {
            svg.zone_screen_centroid(sp.zone, pad).unwrap()
        };

        let b_cents: Vec<_> = (1..=8)

            .map(|i| {

                svg.zone_screen_centroid(PadZone::from("B".to_string()+ &*i.to_string()), pad).unwrap()

            })

            .collect();

        let b_center = b_cents.iter().sum::<Vec2>() / 8.0;
        let b_radius = b_cents.iter().map(|c| c.distance(b_center)).sum::<f32>() / 8.0;
        fn calc(a: i32, b: i32) -> i32 {
            let dist = (a - b + 8) % 8;

            4- dist
        }

        fn parse_circle_num(x: i32)->i32{
            (x % 8) + 1
        }
        let get_zone_pos = |zone: PadZone| {
            svg.zone_screen_centroid(zone, pad).unwrap()
        };
        let base_zone = note.lane+1;
        let total = calc(note.lane as i32,sp.zone.to_id() as i32);
        println!("{}",total);
        let base_zone = PadZone::from("B".to_owned() +&*(parse_circle_num(base_zone as i32)).to_string());
        let base_zone_pos = get_zone_pos(base_zone);
        let bp = (base_zone_pos.y - b_center.y).atan2(base_zone_pos.x - b_center.x);

        let end_zone = PadZone::from("B".to_owned() +&*(parse_circle_num(base_zone as i32+total)).to_string());
        let end_zone_pos = get_zone_pos(end_zone);
        let mut ep = (end_zone_pos.y - b_center.y).atan2(end_zone_pos.x - b_center.x);

        if ep <= bp { ep += std::f32::consts::TAU; }
        path.push(base_zone_pos);
        // 2. base_zone → end_zone（圆弧，步数 = 弧长 / 贴片间距，避免贴片重叠）
        if base_zone!=end_zone {
            let arc_len = b_radius * (ep - bp).abs();
            let spacing = SLIDE_TILE_SPACING * scale;
            let steps = ((arc_len / spacing).ceil() as usize).max(4);
            for i in 1..=steps {
                let ang = bp + (ep - bp) * i as f32 / steps as f32;
                path.push(b_center + vec2(ang.cos(), ang.sin()) * b_radius);
            }
        }
        path.push(end);
    }
}

