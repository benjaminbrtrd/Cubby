//! Global launcher settings (`settings.toml`).

use cubby_common::paths::Paths;
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::util::atomic_write;

/// How the window frame is coloured in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameMode {
    /// Follow the selected instance.
    Instance,
    /// Use [`Settings::frame_color`].
    Fixed,
    /// No frame.
    None,
}

/// What the launcher does when the game starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnLaunch {
    Minimize,
    Keep,
    Close,
}

/// Global Java settings; instances may override them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JavaSettings {
    /// Pick or download a suitable runtime automatically.
    pub auto: bool,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub jvm_args: String,
}

impl Default for JavaSettings {
    fn default() -> Self {
        Self { auto: true, min_memory_mb: 1024, max_memory_mb: 4096, jvm_args: String::new() }
    }
}

/// Default game window settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameSettings {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self { width: 1280, height: 720, fullscreen: false }
    }
}

/// How the launcher reaches the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyMode {
    /// Use the system proxy configuration.
    System,
    /// Never use a proxy.
    None,
    /// Use [`ProxySettings::url`].
    Manual,
}

/// Network proxy settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProxySettings {
    pub mode: ProxyMode,
    /// Proxy URL, used when `mode` is [`ProxyMode::Manual`].
    pub url: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self { mode: ProxyMode::System, url: String::new() }
    }
}

/// Launcher settings. Every field has a default and unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: String,
    /// "night", "day", "contrast" or "system". Older files said "dark" / "light": see [`Settings::load`].
    pub theme: String,
    /// Use Atkinson Hyperlegible instead of the regular UI fonts.
    pub hyperlegible: bool,
    pub reduce_motion: bool,
    /// Check installed content for updates on Modrinth when an instance is opened.
    pub check_updates: bool,
    pub accent: String,
    pub frame_mode: FrameMode,
    pub frame_color: String,
    pub java: JavaSettings,
    pub game: GameSettings,
    pub on_launch: OnLaunch,
    pub download_concurrency: u32,
    pub msa_client_id: Option<String>,
    /// Record play time per instance.
    pub track_play_time: bool,
    /// Show play time in the UI.
    pub show_play_time: bool,
    pub proxy: ProxySettings,
    /// CurseForge API key, if the user provided one.
    pub curseforge_api_key: Option<String>,
    /// The first-run onboarding has been completed.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "system".into(),
            theme: "night".into(),
            hyperlegible: false,
            reduce_motion: false,
            check_updates: true,
            accent: "#FB9168".into(),
            frame_mode: FrameMode::Instance,
            frame_color: "#3D2645".into(),
            java: JavaSettings::default(),
            game: GameSettings::default(),
            on_launch: OnLaunch::Minimize,
            download_concurrency: 16,
            msa_client_id: None,
            track_play_time: true,
            show_play_time: true,
            proxy: ProxySettings::default(),
            curseforge_api_key: None,
            onboarded: false,
        }
    }
}

impl Settings {
    /// Loads `settings.toml`; a missing file yields the defaults. The old theme names "dark" and "light"
    /// are read as "night" and "day".
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.settings_file();
        let mut settings: Self = match std::fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Toml { path: file, message: e.to_string() })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(Error::io(file, e)),
        };
        match settings.theme.as_str() {
            "dark" => settings.theme = "night".into(),
            "light" => settings.theme = "day".into(),
            _ => {}
        }
        Ok(settings)
    }

    /// Saves atomically to `settings.toml`.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let file = paths.settings_file();
        let text = toml::to_string_pretty(self).map_err(|e| Error::Toml { path: file.clone(), message: e.to_string() })?;
        atomic_write(&file, text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn defaults_when_missing_and_round_trip() {
        let paths = Paths::new(temp_dir("settings"));
        let mut s = Settings::load(&paths).unwrap();
        assert_eq!(s, Settings::default());
        assert_eq!(s.accent, "#FB9168");
        assert_eq!(s.java.max_memory_mb, 4096);
        assert_eq!(s.theme, "night");
        assert!(!s.hyperlegible && !s.reduce_motion && s.check_updates);
        s.theme = "contrast".into();
        s.hyperlegible = true;
        s.reduce_motion = true;
        s.check_updates = false;
        s.frame_mode = FrameMode::Fixed;
        s.on_launch = OnLaunch::Close;
        s.msa_client_id = Some("abc".into());
        s.java.jvm_args = "-Xss1M".into();
        s.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), s);
        let text = std::fs::read_to_string(paths.settings_file()).unwrap();
        assert!(text.contains("frameMode = \"fixed\""), "{text}");
        assert!(text.contains("maxMemoryMb"), "{text}");
        assert!(text.contains("reduceMotion = true") && text.contains("checkUpdates = false"), "{text}");
    }

    #[test]
    fn partial_and_unknown_fields() {
        let paths = Paths::new(temp_dir("settings-partial"));
        std::fs::write(paths.settings_file(), "theme = \"light\"\nfuture = 1\n[java]\nmaxMemoryMb = 8192\n").unwrap();
        let s = Settings::load(&paths).unwrap();
        assert_eq!(s.theme, "day");
        assert_eq!(s.java.max_memory_mb, 8192);
        assert_eq!(s.java.min_memory_mb, 1024);
        assert_eq!(s.download_concurrency, 16);
    }

    #[test]
    fn old_files_without_new_keys_load() {
        let paths = Paths::new(temp_dir("settings-old"));
        std::fs::write(paths.settings_file(), "theme = \"dark\"\n[java]\nauto = true\n").unwrap();
        let mut s = Settings::load(&paths).unwrap();
        assert_eq!(s.theme, "night");
        assert!(!s.hyperlegible && !s.reduce_motion && s.check_updates);
        assert!(s.track_play_time && s.show_play_time && !s.onboarded);
        assert_eq!(s.proxy, ProxySettings { mode: ProxyMode::System, url: String::new() });
        assert_eq!(s.curseforge_api_key, None);
        s.proxy = ProxySettings { mode: ProxyMode::Manual, url: "http://p:8080".into() };
        s.curseforge_api_key = Some("k".into());
        s.save(&paths).unwrap();
        let text = std::fs::read_to_string(paths.settings_file()).unwrap();
        assert!(text.contains("mode = \"manual\""), "{text}");
        assert_eq!(Settings::load(&paths).unwrap(), s);
    }

    #[test]
    fn invalid_toml_is_an_error() {
        let paths = Paths::new(temp_dir("settings-bad"));
        std::fs::write(paths.settings_file(), "theme = [").unwrap();
        assert!(matches!(Settings::load(&paths), Err(Error::Toml { .. })));
    }
}
