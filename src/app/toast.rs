//! Toast notification system for the editor.
//!
//! Provides a simple way to show temporary messages to the user,
//! similar to Android toast or web notifications.

use macroquad::prelude::*;

/// Toast severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    /// Informational message (blue)
    Info,
    /// Success message (green)
    Success,
    /// Warning message (yellow)
    Warning,
    /// Error message (red)
    Error,
}

impl ToastLevel {
    pub fn color(&self) -> Color {
        match self {
            ToastLevel::Info => Color::new(0.2, 0.5, 0.9, 1.0),
            ToastLevel::Success => Color::new(0.2, 0.7, 0.3, 1.0),
            ToastLevel::Warning => Color::new(0.9, 0.7, 0.1, 1.0),
            ToastLevel::Error => Color::new(0.9, 0.2, 0.2, 1.0),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToastLevel::Info => "ℹ",
            ToastLevel::Success => "✓",
            ToastLevel::Warning => "⚠",
            ToastLevel::Error => "✗",
        }
    }
}

/// A single toast notification
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created_at: f64,
    pub duration: f32,
}

impl Toast {
    pub fn new(message: impl Into<String>, level: ToastLevel) -> Self {
        Self {
            message: message.into(),
            level,
            created_at: get_time(),
            duration: 3.0,
        }
    }

    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }

    pub fn is_expired(&self) -> bool {
        (get_time() - self.created_at) as f32 >= self.duration
    }

    pub fn alpha(&self) -> f32 {
        let elapsed = (get_time() - self.created_at) as f32;
        if elapsed < self.duration - 0.5 {
            1.0
        } else {
            ((self.duration - elapsed) / 0.5).clamp(0.0, 1.0)
        }
    }
}

/// Toast manager
pub struct ToastManager {
    toasts: Vec<Toast>,
    max_visible: usize,
}

impl ToastManager {
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            max_visible: 5,
        }
    }

    /// Add a new toast
    pub fn add(&mut self, message: impl Into<String>, level: ToastLevel) {
        self.toasts.push(Toast::new(message, level));
        // Keep only the most recent toasts
        if self.toasts.len() > self.max_visible * 2 {
            self.toasts.drain(0..self.toasts.len() - self.max_visible);
        }
    }

    /// Add an info toast
    pub fn info(&mut self, message: impl Into<String>) {
        self.add(message, ToastLevel::Info);
    }

    /// Add a success toast
    pub fn success(&mut self, message: impl Into<String>) {
        self.add(message, ToastLevel::Success);
    }

    /// Add a warning toast
    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(message, ToastLevel::Warning);
    }

    /// Add an error toast
    pub fn error(&mut self, message: impl Into<String>) {
        self.add(message, ToastLevel::Error);
    }

    /// Update toasts (remove expired ones)
    pub fn update(&mut self) {
        self.toasts.retain(|t| !t.is_expired());
    }

    /// Render all active toasts
    pub fn draw(&self) {
        let screen_w = screen_width();
        let _screen_h = screen_height();
        let toast_w = 300.0_f32.min(screen_w - 40.0);
        let toast_h = 40.0;
        let padding = 10.0;
        let start_x = screen_w - toast_w - 20.0;
        let mut y = 60.0; // Start below toolbar

        for toast in self.toasts.iter().take(self.max_visible) {
            let alpha = toast.alpha();
            if alpha <= 0.0 {
                continue;
            }

            let bg_color = Color::new(0.15, 0.15, 0.18, 0.95 * alpha);
            let border_color = Color::new(
                toast.level.color().r,
                toast.level.color().g,
                toast.level.color().b,
                0.8 * alpha,
            );

            // Background
            draw_rectangle(start_x, y, toast_w, toast_h, bg_color);
            draw_rectangle_lines(start_x, y, toast_w, toast_h, 2.0, border_color);

            // Icon
            let icon_color = Color::new(
                toast.level.color().r,
                toast.level.color().g,
                toast.level.color().b,
                alpha,
            );
            draw_text(
                toast.level.icon(),
                start_x + padding,
                y + toast_h * 0.65,
                20.0,
                icon_color,
            );

            // Message text
            let text_color = Color::new(0.9, 0.9, 0.9, alpha);
            let text_x = start_x + padding + 24.0;
            let max_text_w = toast_w - padding * 2.0 - 24.0;

            // Truncate text if too long
            let display_text = truncate_text(&toast.message, max_text_w, 14.0);
            draw_text(&display_text, text_x, y + toast_h * 0.65, 14.0, text_color);

            // Progress bar
            let elapsed = (get_time() - toast.created_at) as f32;
            let progress = 1.0 - (elapsed / toast.duration).clamp(0.0, 1.0);
            let bar_color = Color::new(
                toast.level.color().r,
                toast.level.color().g,
                toast.level.color().b,
                0.5 * alpha,
            );
            draw_rectangle(
                start_x,
                y + toast_h - 3.0,
                toast_w * progress,
                3.0,
                bar_color,
            );

            y += toast_h + 8.0;
        }
    }

    /// Clear all toasts
    pub fn clear(&mut self) {
        self.toasts.clear();
    }
}

/// Truncate text to fit within a given width
fn truncate_text(text: &str, max_width: f32, font_size: f32) -> String {
    let char_width = font_size * 0.5; // Approximate
    let max_chars = (max_width / char_width) as usize;
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!("{}...", &text[..max_chars.saturating_sub(3)])
    }
}

/// Convenience macro for creating toasts
#[macro_export]
macro_rules! toast {
    ($manager:expr, info, $($arg:tt)*) => {
        $manager.info(format!($($arg)*))
    };
    ($manager:expr, success, $($arg:tt)*) => {
        $manager.success(format!($($arg)*))
    };
    ($manager:expr, warning, $($arg:tt)*) => {
        $manager.warning(format!($($arg)*))
    };
    ($manager:expr, error, $($arg:tt)*) => {
        $manager.error(format!($($arg)*))
    };
}
