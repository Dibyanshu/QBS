use crate::{
    browser::{BrowserId, BrowserSnapshot},
    config::{Config, SwitchMode, ThemePreference},
};
use anyhow::Result;
use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    Switch(BrowserId),
    SetMode(SwitchMode),
    AcknowledgeExperimental,
    OpenSettings,
    Refresh,
    ToggleLock,
    ToggleStartup,
    SetTheme(ThemePreference),
    SetOpacity(u8),
    ShowHide,
    About,
    Quit,
}

pub struct ContextMenu {
    pub menu: Menu,
    chrome: MenuItem,
    edge: MenuItem,
    supported: CheckMenuItem,
    experimental: CheckMenuItem,
    acknowledge: CheckMenuItem,
    settings: MenuItem,
    refresh: MenuItem,
    lock: CheckMenuItem,
    startup: CheckMenuItem,
    themes: Vec<(CheckMenuItem, ThemePreference)>,
    opacities: Vec<(CheckMenuItem, u8)>,
    show_hide: MenuItem,
    about: MenuItem,
    quit: MenuItem,
}

impl ContextMenu {
    pub fn new(config: &Config, snapshot: &BrowserSnapshot) -> Result<Self> {
        let menu = Menu::new();
        let chrome = MenuItem::new(
            "Switch to Google Chrome",
            snapshot
                .browser(BrowserId::Chrome)
                .is_some_and(|b| b.installed),
            None,
        );
        let edge = MenuItem::new(
            "Switch to Microsoft Edge",
            snapshot
                .browser(BrowserId::Edge)
                .is_some_and(|b| b.installed),
            None,
        );
        let mode = Submenu::new("Switching mode", true);
        let supported = CheckMenuItem::new(
            "Supported confirmation",
            true,
            config.switch_mode == SwitchMode::Supported,
            None,
        );
        let experimental = CheckMenuItem::new(
            "Experimental silent",
            true,
            config.switch_mode == SwitchMode::Experimental,
            None,
        );
        let acknowledge = CheckMenuItem::new(
            "I understand the experimental risk",
            true,
            config.experimental_warning_acknowledged,
            None,
        );
        mode.append(&supported)?;
        mode.append(&experimental)?;
        mode.append(&PredefinedMenuItem::separator())?;
        mode.append(&acknowledge)?;
        let settings = MenuItem::new("Open Windows Default Apps", true, None);
        let refresh = MenuItem::new("Refresh status", true, None);
        let lock = CheckMenuItem::new("Lock position", true, config.window_locked, None);
        let startup =
            CheckMenuItem::new("Start with Windows", true, config.start_with_windows, None);
        let theme_menu = Submenu::new("Theme", true);
        let themes = [
            ("System", ThemePreference::System),
            ("Light", ThemePreference::Light),
            ("Dark", ThemePreference::Dark),
        ]
        .into_iter()
        .map(|(label, theme)| {
            (
                CheckMenuItem::new(label, true, config.theme == theme, None),
                theme,
            )
        })
        .collect::<Vec<_>>();
        for (item, _) in &themes {
            theme_menu.append(item)?;
        }
        let opacity_menu = Submenu::new("Opacity", true);
        let current_opacity = (config.opacity * 100.0).round() as u8;
        let opacities = [60, 75, 90, 100]
            .into_iter()
            .map(|value| {
                (
                    CheckMenuItem::new(
                        format!("{value}%"),
                        true,
                        current_opacity.abs_diff(value) < 8,
                        None,
                    ),
                    value,
                )
            })
            .collect::<Vec<_>>();
        for (item, _) in &opacities {
            opacity_menu.append(item)?;
        }
        let show_hide = MenuItem::new("Show / Hide widget", true, None);
        let about = MenuItem::new(
            format!("About QBS v{}", env!("CARGO_PKG_VERSION")),
            true,
            None,
        );
        let quit = MenuItem::new("Quit", true, None);
        for item in [
            &chrome as &dyn muda::IsMenuItem,
            &edge,
            &PredefinedMenuItem::separator(),
            &mode,
            &settings,
            &refresh,
            &PredefinedMenuItem::separator(),
            &lock,
            &startup,
            &theme_menu,
            &opacity_menu,
            &PredefinedMenuItem::separator(),
            &show_hide,
            &about,
            &quit,
        ] {
            menu.append(item)?;
        }
        Ok(Self {
            menu,
            chrome,
            edge,
            supported,
            experimental,
            acknowledge,
            settings,
            refresh,
            lock,
            startup,
            themes,
            opacities,
            show_hide,
            about,
            quit,
        })
    }

    pub fn action_for(&self, id: &muda::MenuId) -> Option<MenuAction> {
        if id == self.chrome.id() {
            Some(MenuAction::Switch(BrowserId::Chrome))
        } else if id == self.edge.id() {
            Some(MenuAction::Switch(BrowserId::Edge))
        } else if id == self.supported.id() {
            Some(MenuAction::SetMode(SwitchMode::Supported))
        } else if id == self.experimental.id() {
            Some(MenuAction::SetMode(SwitchMode::Experimental))
        } else if id == self.acknowledge.id() {
            Some(MenuAction::AcknowledgeExperimental)
        } else if id == self.settings.id() {
            Some(MenuAction::OpenSettings)
        } else if id == self.refresh.id() {
            Some(MenuAction::Refresh)
        } else if id == self.lock.id() {
            Some(MenuAction::ToggleLock)
        } else if id == self.startup.id() {
            Some(MenuAction::ToggleStartup)
        } else if id == self.show_hide.id() {
            Some(MenuAction::ShowHide)
        } else if id == self.about.id() {
            Some(MenuAction::About)
        } else if id == self.quit.id() {
            Some(MenuAction::Quit)
        } else if let Some((_, theme)) = self.themes.iter().find(|(item, _)| id == item.id()) {
            Some(MenuAction::SetTheme(*theme))
        } else {
            self.opacities
                .iter()
                .find(|(item, _)| id == item.id())
                .map(|(_, opacity)| MenuAction::SetOpacity(*opacity))
        }
    }
}
