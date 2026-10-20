use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteSkin {
    #[default]
    Classic,
    Lambdadx,
}

impl NoteSkin {
    pub const ALL: [Self; 2] = [Self::Classic, Self::Lambdadx];

    pub fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Lambdadx => "lambdadx",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Lambdadx => "LambdaDX",
        }
    }
}

pub const WIFI_STRIP_Y: [f32; 11] = [
    429.0, 354.0, 286.5, 217.5, 147.0, 61.5, -25.5, -111.0, -196.5, -277.5, -373.5,
];

/// Fan art is registered on a 480-unit ring, independently of arrow thickness.
pub fn wifi_texture_scale(skin: NoteSkin, target_radius: f32, scale: f32, tile_scale: f32) -> f32 {
    match skin {
        NoteSkin::Classic => scale * tile_scale,
        NoteSkin::Lambdadx => target_radius / 480.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_reference_size_is_independent_of_arrow_scale() {
        for radius in [100.0, 240.0, 480.0] {
            for tile_scale in [0.52, 0.68, 1.5] {
                let scale = wifi_texture_scale(NoteSkin::Lambdadx, radius, radius / 300.0, tile_scale);
                assert!((678.0 * scale - 678.0 * radius / 480.0).abs() < 0.0001);
            }
        }
        assert_eq!(wifi_texture_scale(NoteSkin::Classic, 480.0, 2.0, 0.5), 1.0);
    }
}
