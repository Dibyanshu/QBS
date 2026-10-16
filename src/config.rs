use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub const CONFIG_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SwitchMode {
    #[default]
    Supported,
    Experimental,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

fn default_opacity() -> f32 {
    0.94
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub switch_mode: SwitchMode,
    pub experimental_warning_acknowledged: bool,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_locked: bool,
    pub always_on_top: bool,
    pub theme: ThemePreference,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    pub start_with_windows: bool,
    pub last_selected_browser: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            switch_mode: SwitchMode::Supported,
            experimental_warning_acknowledged: false,
            window_x: None,
            window_y: None,
            window_locked: false,
            always_on_top: true,
            theme: ThemePreference::System,
            opacity: default_opacity(),
            start_with_windows: false,
            last_selected_browser: None,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("QuickBrowserSwitcher")
        .join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    let Ok(text) = fs::read_to_string(path) else {
        return Config::default();
    };
    let mut config: Config = toml::from_str(&text).unwrap_or_default();
    config.version = CONFIG_VERSION;
    config.opacity = config.opacity.clamp(0.55, 1.0);
    config
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    let parent = path.parent().context("config path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, toml::to_string_pretty(config)?)?;
    fs::rename(&temporary, &path).or_else(|_| {
        let _ = fs::remove_file(&path);
        fs::rename(&temporary, &path)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe() {
        let c = Config::default();
        assert_eq!(c.switch_mode, SwitchMode::Supported);
        assert!(!c.experimental_warning_acknowledged);
        assert!(!c.start_with_windows);
        assert!(c.always_on_top);
    }

    #[test]
    fn old_or_partial_config_migrates() {
        let c: Config = toml::from_str("window_locked = true").unwrap();
        assert!(c.window_locked);
        assert_eq!(c.switch_mode, SwitchMode::Supported);
        assert_eq!(c.version, CONFIG_VERSION);
        assert!(c.always_on_top);
    }
}
