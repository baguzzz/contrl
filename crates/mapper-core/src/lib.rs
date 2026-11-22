pub mod actions;
pub mod gyro;

use glam::Vec2;

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct StickConfig {
    pub inner_deadzone: f32,
    pub outer_deadzone: f32,
    pub curve_exponent: f32,
}

impl Default for StickConfig {
    fn default() -> Self {
        Self {
            inner_deadzone: 0.15,
            outer_deadzone: 0.95,
            curve_exponent: 1.0,
        }
    }
}

pub fn process_stick(raw_x: f32, raw_y: f32, cfg: &StickConfig) -> Vec2 {
    let raw = Vec2::new(raw_x, raw_y);
    let magnitude = raw.length();

    if magnitude <= cfg.inner_deadzone {
        return Vec2::ZERO;
    }

    let normalized_mag =
        ((magnitude - cfg.inner_deadzone)
            / (cfg.outer_deadzone - cfg.inner_deadzone))
            .clamp(0.0, 1.0);

    let curved_mag = normalized_mag.powf(cfg.curve_exponent);
    raw.normalize_or_zero() * curved_mag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deadzone_filters_drift() {
        let cfg = StickConfig {
            inner_deadzone: 0.15,
            outer_deadzone: 0.95,
            curve_exponent: 1.0,
        };

        let result = process_stick(0.05, -0.08, &cfg);

        assert_eq!(result, Vec2::ZERO);
    }

    #[test]
    fn test_outer_deadzone_reaches_full_output() {
        let cfg = StickConfig {
            inner_deadzone: 0.15,
            outer_deadzone: 0.95,
            curve_exponent: 1.0,
        };

        let result = process_stick(1.0, 0.0, &cfg);

        assert!((result.x - 1.0).abs() < 0.0001);
    }
}