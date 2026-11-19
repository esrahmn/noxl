/// An RGBA color with 8-bit channels.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Errors returned when parsing a color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorError {
    InvalidHexLength,
    InvalidHexDigit,
}

impl Color {
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const RED: Self = Self::rgba(255, 0, 0, 255);
    pub const GREEN: Self = Self::rgba(0, 255, 0, 255);
    pub const BLUE: Self = Self::rgba(0, 0, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    /// Creates a color from a hexadecimal string.
    /// Supports RRGGBB and RRGGBBAA, with optional # prefix.
    pub fn from_hex(hex: &str) -> Result<Self, ColorError> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);

        if hex.len() != 6 && hex.len() != 8 {
            return Err(ColorError::InvalidHexLength);
        }

        if !hex.is_ascii() {
            return Err(ColorError::InvalidHexDigit);
        }

        let value = u32::from_str_radix(hex, 16).map_err(|_| ColorError::InvalidHexDigit)?;

        if hex.len() == 6 {
            Ok(Self::rgba(
                ((value >> 16) & 0xFF) as u8,
                ((value >> 8) & 0xFF) as u8,
                (value & 0xFF) as u8,
                255,
            ))
        } else {
            Ok(Self::rgba(
                ((value >> 24) & 0xFF) as u8,
                ((value >> 16) & 0xFF) as u8,
                ((value >> 8) & 0xFF) as u8,
                (value & 0xFF) as u8,
            ))
        }
    }

    /// Converts channels into normalized floating-point values.
    pub fn to_rgba_f32(self) -> [f32; 4] {
        const SCALE: f32 = 1.0 / 255.0;

        [
            self.r as f32 * SCALE,
            self.g as f32 * SCALE,
            self.b as f32 * SCALE,
            self.a as f32 * SCALE,
        ]
    }

    /// Returns this color with a replaced alpha value.
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }

    /// Multiplies the current opacity by the supplied factor.
    pub fn with_opacity(self, opacity: f32) -> Self {
        if !opacity.is_finite() {
            return self.with_alpha(0);
        }

        let opacity = opacity.clamp(0.0, 1.0);
        let alpha = (self.a as f32 * opacity).round() as u8;

        self.with_alpha(alpha)
    }

    /// Converts the color into a hexadecimal string.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }

    /// Alpha-composites this foreground color over a background.
    pub fn blend_over(self, background: Self) -> Self {
        let foreground_alpha = self.a as f32 / 255.0;
        let background_alpha = background.a as f32 / 255.0;

        let output_alpha = foreground_alpha + background_alpha * (1.0 - foreground_alpha);

        if output_alpha <= 0.0 {
            return Self::TRANSPARENT;
        }

        let blend_channel = |foreground: u8, background: u8| -> u8 {
            let foreground = foreground as f32 / 255.0;
            let background = background as f32 / 255.0;

            let value = (foreground * foreground_alpha
                + background * background_alpha * (1.0 - foreground_alpha))
                / output_alpha;

            (value * 255.0).round().clamp(0.0, 255.0) as u8
        };

        Self::rgba(
            blend_channel(self.r, background.r),
            blend_channel(self.g, background.g),
            blend_channel(self.b, background.b),
            (output_alpha * 255.0).round() as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_creates_color_correctly() {
        assert_eq!(
            Color::rgba(255, 128, 64, 200),
            Color {
                r: 255,
                g: 128,
                b: 64,
                a: 200,
            }
        );
    }

    #[test]
    fn rgb_creates_opaque_color() {
        assert_eq!(Color::rgb(255, 128, 64).a, 255);
    }

    #[test]
    fn predefined_colors_are_correct() {
        assert_eq!(Color::BLACK, Color::rgba(0, 0, 0, 255));
        assert_eq!(Color::WHITE, Color::rgba(255, 255, 255, 255));
        assert_eq!(Color::RED, Color::rgba(255, 0, 0, 255));
        assert_eq!(Color::GREEN, Color::rgba(0, 255, 0, 255));
        assert_eq!(Color::BLUE, Color::rgba(0, 0, 255, 255));
        assert_eq!(Color::TRANSPARENT, Color::rgba(0, 0, 0, 0));
    }

    #[test]
    fn parses_hex_rgb() {
        assert_eq!(
            Color::from_hex("#FF8040"),
            Ok(Color::rgba(255, 128, 64, 255))
        );
    }

    #[test]
    fn parses_hex_rgba() {
        assert_eq!(
            Color::from_hex("FF804080"),
            Ok(Color::rgba(255, 128, 64, 128))
        );
    }

    #[test]
    fn rejects_invalid_hex_length() {
        assert_eq!(Color::from_hex("#FFF"), Err(ColorError::InvalidHexLength));
    }

    #[test]
    fn rejects_invalid_hex_digit() {
        assert_eq!(Color::from_hex("#GGGGGG"), Err(ColorError::InvalidHexDigit));
    }

    #[test]
    fn converts_to_normalized_float() {
        assert_eq!(
            Color::rgba(255, 128, 0, 255).to_rgba_f32(),
            [1.0, 128.0 / 255.0, 0.0, 1.0]
        );
    }

    #[test]
    fn replaces_alpha() {
        assert_eq!(Color::WHITE.with_alpha(128).a, 128);
    }

    #[test]
    fn applies_opacity() {
        assert_eq!(Color::rgba(255, 0, 0, 255).with_opacity(0.5).a, 128);
    }

    #[test]
    fn clamps_opacity() {
        assert_eq!(Color::WHITE.with_opacity(2.0).a, 255);
        assert_eq!(Color::WHITE.with_opacity(-1.0).a, 0);
    }

    #[test]
    fn serializes_to_hex() {
        assert_eq!(Color::rgba(255, 128, 64, 200).to_hex(), "#FF8040C8");
    }

    #[test]
    fn blends_opaque_foreground_over_background() {
        let foreground = Color::RED;
        let result = foreground.blend_over(Color::BLUE);
        assert_eq!(result, Color::RED);
    }

    #[test]
    fn blends_transparent_foreground_over_background() {
        let foreground = Color::TRANSPARENT;
        let result = foreground.blend_over(Color::BLUE);
        assert_eq!(result, Color::BLUE);
    }

    #[test]
    fn blends_two_transparent_colors() {
        assert_eq!(
            Color::TRANSPARENT.blend_over(Color::TRANSPARENT),
            Color::TRANSPARENT
        );
    }
}
