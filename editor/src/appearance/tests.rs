use super::*;

#[test]
fn old_appearance_files_enable_the_default_watermark() {
    let mut settings: Settings =
        serde_json::from_str(r#"{"background":false,"opacity":45}"#).unwrap();
    settings.normalize();
    assert!(!settings.background);
    assert_eq!(settings.opacity, 45.0);
    assert!(settings.watermark.enabled);
    assert_eq!(settings.watermark.text, "禁止泄漏! Don't Leak");
    assert_eq!(settings.watermark, Watermark::default());
}

#[test]
fn watermark_controls_autosave_and_survive_restart_without_a_background_image() {
    const CHILD: &str = "LAMBDADX_TEST_WATERMARK_CACHE";
    if std::env::var_os(CHILD).is_none() {
        let dir = std::env::temp_dir().join(format!("lambdadx-watermark-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "appearance::tests::watermark_controls_autosave_and_survive_restart_without_a_background_image", "--nocapture"])
            .env(CHILD, &dir).env("MAI2_CACHE_DIR", &dir).env("MAI2_DATA_DIR", &dir)
            .output();
        let _ = std::fs::remove_dir_all(&dir);
        let result = result.unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let mut appearance = Appearance::default();
    appearance.settings.background = false;
    let ctx = egui::Context::default();
    crate::ui::style::apply(&ctx);
    let frame = |appearance: &mut Appearance, events: Vec<egui::Event>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| appearance.controls(ui));
            },
        )
    };
    frame(&mut appearance, Vec::new());
    let output = frame(&mut appearance, Vec::new());
    let at = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "背景水印" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("watermark toggle is visible in appearance settings");
    for pressed in [true, false] {
        frame(
            &mut appearance,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(!appearance.settings.watermark.enabled);
    appearance.settings.watermark.text = "测试 Private".into();
    appearance.flush(true);
    assert!(
        platform::read_cache_text(FILE).is_err(),
        "dragging must defer writes"
    );
    appearance.flush(false);
    assert!(appearance.save_error.is_none());
    let mut restored = Appearance::default();
    restored.load();
    assert_eq!(restored.settings, appearance.settings);
    assert!(restored.loading.is_none());
    assert!(!restored.settings.watermark.enabled);
    restored.settings.watermark = Watermark::default();
    restored.flush(false);
    let mut restarted = Appearance::default();
    restarted.load();
    assert_eq!(restarted.settings.watermark, Watermark::default());
}
