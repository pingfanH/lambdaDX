//! Editor-only backdrop and persisted surface settings.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::path::{Path, PathBuf};

use egui_macroquad::egui;
use macroquad::prelude as mq;
use serde::{Deserialize, Serialize};

use crate::{app::platform, theme};

const FILE: &str = "editor_appearance.json";
const DEFAULT_IMAGE: &str = "editor_background.jpg";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    transparent: bool,
    opacity: f32,
    background: bool,
    brightness: f32,
    image: String,
    dim_inactive_during_edit: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            transparent: true,
            opacity: 68.0,
            background: true,
            brightness: 55.0,
            image: DEFAULT_IMAGE.into(),
            dim_inactive_during_edit: false,
        }
    }
}

impl Settings {
    fn normalize(&mut self) {
        let defaults = Self::default();
        for (value, fallback) in [
            (&mut self.opacity, defaults.opacity),
            (&mut self.brightness, defaults.brightness),
        ] {
            *value = if value.is_finite() {
                value.clamp(0.0, 100.0)
            } else {
                fallback
            };
        }
        if self.image.trim().is_empty() {
            self.image = DEFAULT_IMAGE.into();
        }
    }
}

type Loaded = Result<Option<(String, image::RgbaImage)>, String>;

pub struct Appearance {
    settings: Settings,
    saved: Settings,
    draft_path: String,
    texture: Option<mq::Texture2D>,
    loading: Option<Receiver<Loaded>>,
    error: Option<String>,
    save_error: Option<String>,
    pub browse_requested: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        let settings = Settings::default();
        Self {
            draft_path: settings.image.clone(),
            saved: settings.clone(),
            settings,
            texture: None,
            loading: None,
            error: None,
            save_error: None,
            browse_requested: false,
        }
    }
}

impl Appearance {
    pub fn load(&mut self) {
        match platform::read_cache_text(FILE).and_then(|text| {
            serde_json::from_str::<Settings>(&text).map_err(|error| error.to_string())
        }) {
            Ok(mut settings) => {
                settings.normalize();
                self.draft_path = settings.image.clone();
                self.saved = settings.clone();
                self.settings = settings;
            }
            Err(error) => eprintln!("[appearance] using defaults: {error}"),
        }
        if self.settings.background {
            self.request_image(self.settings.image.clone());
        }
    }

    pub fn opacity(&self) -> f32 {
        if self.settings.transparent {
            self.settings.opacity / 100.0
        } else {
            1.0
        }
    }

    pub fn dim_inactive_during_edit(&self) -> bool {
        self.settings.dim_inactive_during_edit
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        use crate::ui::style::icon;
        use lucide_icons::Icon;

        ui.checkbox(&mut self.settings.transparent, "透明界面");
        ui.checkbox(&mut self.settings.dim_inactive_during_edit, "操作时淡化其他分区");
        ui.add_enabled(
            self.settings.transparent,
            egui::Slider::new(&mut self.settings.opacity, 0.0..=100.0)
                .suffix("%")
                .text("界面不透明度"),
        );
        if ui
            .checkbox(&mut self.settings.background, "背景图片")
            .changed()
            && self.settings.background
            && self.texture.is_none()
            && self.loading.is_none()
        {
            self.request_image(self.settings.image.clone());
        }
        ui.add_enabled(
            self.settings.background,
            egui::Slider::new(&mut self.settings.brightness, 0.0..=100.0)
                .suffix("%")
                .text("图片亮度"),
        );
        ui.horizontal(|ui| {
            ui.add_enabled_ui(self.loading.is_none(), |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.draft_path)
                        .desired_width((ui.available_width() - 84.0).clamp(80.0, 240.0)),
                );
                let apply = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if icon(ui, Icon::Check, "加载图片路径", false).clicked() || apply {
                    let path = self.draft_path.trim().to_string();
                    if !path.is_empty() {
                        self.request_image(path);
                    }
                }
                #[cfg(target_os = "macos")]
                if icon(ui, Icon::FolderOpen, "选择背景图片", false).clicked() {
                    self.browse_requested = true;
                }
                if icon(ui, Icon::RotateCcw, "恢复默认背景图片", false).clicked() {
                    self.request_image(DEFAULT_IMAGE.into());
                }
            });
        });
        if self.loading.is_some() {
            ui.spinner();
        }
        for error in [&self.error, &self.save_error].into_iter().flatten() {
            ui.colored_label(theme::ACCENT_RED, error);
        }
        if self.save_error.is_some() && icon(ui, Icon::RotateCw, "重试保存外观", false).clicked()
        {
            self.save();
        }
    }

    pub fn flush(&mut self, pointer_down: bool) {
        // Also runs with settings closed; failed writes require an explicit retry.
        if self.settings != self.saved && self.save_error.is_none() && !pointer_down {
            self.save();
        }
    }

    fn save(&mut self) {
        let result = (|| {
            let text = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
            let temp = platform::write_cache_text("editor_appearance.tmp", &text)?;
            std::fs::rename(&temp, temp.with_file_name(FILE)).map_err(|e| e.to_string())
        })();
        match result {
            Ok(()) => {
                self.saved = self.settings.clone();
                self.save_error = None;
            }
            Err(error) => self.save_error = Some(format!("保存外观失败: {error}")),
        }
    }

    pub fn image_path(&self) -> PathBuf {
        let path = PathBuf::from(&self.settings.image);
        if path.is_absolute() { path } else { platform::asset_dir().join(path) }
    }

    pub fn request_image(&mut self, path: String) {
        if self.loading.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.loading = Some(rx);
        self.error = None;
        std::thread::spawn(move || {
            let result = (|| {
                let image = decode(&path)?;
                Ok(Some((path, image)))
            })();
            let _ = tx.send(result);
        });
    }

    /// Poll on the render thread; file access and decoding never run here.
    pub fn update(&mut self) {
        let Some(rx) = &self.loading else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("图片加载线程已退出".into()),
        };
        self.loading = None;
        match result {
            Ok(Some((path, image))) => {
                let texture = mq::Texture2D::from_rgba8(
                    image.width() as u16,
                    image.height() as u16,
                    image.as_raw(),
                );
                texture.set_filter(mq::FilterMode::Linear);
                self.texture = Some(texture);
                self.draft_path = path.clone();
                self.settings.image = path;
            }
            Ok(None) => {}
            Err(error) => self.error = Some(format!("背景图片: {error}")),
        }
    }

    pub fn draw(&self) {
        if !self.settings.background {
            return;
        }
        let Some(texture) = &self.texture else {
            return;
        };
        let screen = mq::vec2(mq::screen_width(), mq::screen_height());
        let source = cover_source(mq::vec2(texture.width(), texture.height()), screen);
        let light = self.settings.brightness / 100.0;
        mq::draw_texture_ex(
            texture,
            0.0,
            0.0,
            mq::Color::new(light, light, light, 1.0),
            mq::DrawTextureParams {
                dest_size: Some(screen),
                source: Some(source),
                ..Default::default()
            },
        );
    }

    /// Restore exactly the editor backdrop, not a solid mask, outside the Pad.
    /// The caller keeps the preview scissor active for all three layers.
    pub fn draw_pad_surround(&self, pad: &crate::app::types::PadGeom, surface: mq::Color) {
        let screen = mq::vec2(mq::screen_width(), mq::screen_height());
        let radius = pad.outer_r * crate::app::params::pad_circle_scale().max(1.0);
        let outer = mq::vec2(pad.cx, pad.cy).length() + screen.length() + radius;
        let mut mesh = crate::player::render::pad::annulus_mesh(
            pad.cx, pad.cy, radius, outer, theme::to_mq(theme::BG_WINDOW),
        );
        mq::draw_mesh(&mesh);
        if let Some(texture) = self.texture.as_ref().filter(|_| self.settings.background) {
            let source = cover_source(mq::vec2(texture.width(), texture.height()), screen);
            let light = self.settings.brightness / 100.0;
            for vertex in &mut mesh.vertices {
                vertex.uv = mq::vec2(
                    (source.x + vertex.position.x / screen.x * source.w) / texture.width(),
                    (source.y + vertex.position.y / screen.y * source.h) / texture.height(),
                );
                vertex.color = mq::Color::new(light, light, light, 1.0).into();
            }
            mesh.texture = Some(texture.clone());
            mq::draw_mesh(&mesh);
        }
        if surface.a > 0.0 {
            mesh.texture = None;
            for vertex in &mut mesh.vertices {
                vertex.color = surface.into();
            }
            mq::draw_mesh(&mesh);
        }
    }
}

fn decode(path: &str) -> Result<image::RgbaImage, String> {
    let path = Path::new(path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        platform::asset_dir().join(path)
    };
    decode_file(&path, 4096)
}

fn decode_file(path: &Path, max_side: u32) -> Result<image::RgbaImage, String> {
    let mut reader = image::io::Reader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?;
    let image = if image.width().max(image.height()) > max_side {
        image.thumbnail(max_side, max_side)
    } else {
        image
    };
    Ok(image.into_rgba8())
}

#[derive(Default)]
pub struct CoverArt {
    revision: Option<u64>,
    source: Option<PathBuf>,
    loading: Option<Receiver<Result<Option<image::RgbaImage>, String>>>,
}

impl CoverArt {
    pub fn update(
        &mut self,
        source: Option<&crate::io::ChartSource>,
        app: &mut crate::player::state::PadPreviewState,
    ) -> Option<String> {
        let source = source.map(|source| source.path.as_path());
        if self.revision != Some(app.document_revision()) || self.source.as_deref() != source {
            self.revision = Some(app.document_revision());
            self.source = source.map(Path::to_path_buf);
            // Dropping the old receiver prevents a previous song's late result
            // from replacing the current cover after a rapid chart switch.
            self.loading = None;
            app.cover_texture = None;
            if let Some(folder) = source.and_then(Path::parent).map(Path::to_path_buf) {
                let (tx, rx) = mpsc::channel();
                self.loading = Some(rx);
                std::thread::spawn(move || {
                    let result = crate::app::chart::find_cover_in_dir(&folder)
                        .map(|path| decode_file(&path, 1024)
                            .map_err(|error| format!("{}: {error}", path.display())))
                        .transpose();
                    let _ = tx.send(result);
                });
            }
        }
        let result = match self.loading.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("封面加载线程已退出".into()),
        };
        self.loading = None;
        match result {
            Ok(Some(image)) => {
                let texture = mq::Texture2D::from_rgba8(
                    image.width() as u16, image.height() as u16, image.as_raw(),
                );
                texture.set_filter(mq::FilterMode::Linear);
                app.cover_texture = Some(texture);
                None
            }
            Ok(None) => None,
            Err(error) => Some(format!("读取封面失败: {error}")),
        }
    }
}

fn cover_source(image: mq::Vec2, screen: mq::Vec2) -> mq::Rect {
    let scale = (screen.x / image.x)
        .max(screen.y / image.y)
        .max(f32::EPSILON);
    let size = screen / scale;
    mq::Rect::new(
        (image.x - size.x) * 0.5,
        (image.y - size.y) * 0.5,
        size.x,
        size.y,
    )
}
