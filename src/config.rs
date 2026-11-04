use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyChoice {
    #[default]
    ShiftBackspace,
    PrtScn,
    ShiftPrtScn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Yellow,
    Blue,
    Green,
    Red,
    Black,
    White,
}

impl Color {
    pub const ALL: [Color; 6] = [
        Color::Yellow,
        Color::Blue,
        Color::Green,
        Color::Red,
        Color::Black,
        Color::White,
    ];

    /// Cor em formato COLORREF do Win32 (0x00BBGGRR).
    pub fn colorref(self) -> u32 {
        match self {
            Color::Yellow => 0x0000_FFFF,
            Color::Blue => 0x00FF_0000,
            Color::Green => 0x0000_C000,
            Color::Red => 0x0000_00FF,
            Color::Black => 0x0000_0000,
            Color::White => 0x00FF_FFFF,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey: HotkeyChoice,
    pub start_with_windows: bool,
    pub stroke_width: u32,
    pub text_size: u32,
    pub default_color: Color,
    pub text_background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: HotkeyChoice::ShiftBackspace,
            start_with_windows: false,
            stroke_width: 3,
            text_size: 18,
            default_color: Color::Red,
            text_background: false,
        }
    }
}

impl Settings {
    /// Nunca falha: arquivo ausente ou inválido vira o padrão.
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let s = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_prd() {
        let s = Settings::default();
        assert_eq!(s.hotkey, HotkeyChoice::ShiftBackspace);
        assert!(!s.start_with_windows);
        assert_eq!(s.stroke_width, 3);
        assert_eq!(s.text_size, 18);
        assert_eq!(s.default_color, Color::Red);
        assert!(!s.text_background);
    }

    #[test]
    fn blue_is_pure_blue_colorref() {
        assert_eq!(Color::Blue.colorref(), 0x00FF_0000);
    }

    #[test]
    fn invalid_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("shot_cfg_invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "{ isto não é json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
    }

    #[test]
    fn round_trip_save_load() {
        let dir = std::env::temp_dir().join("shot_cfg_rt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        let s = Settings {
            stroke_width: 7,
            default_color: Color::Blue,
            ..Settings::default()
        };
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
    }
}
