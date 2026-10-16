mod experimental;

use crate::{
    browser::{Browser, BrowserId, BrowserSnapshot},
    config::SwitchMode,
    platform,
};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchResultKind {
    Changed,
    ConfirmationRequired,
    UnsupportedWindows,
    VerificationFailed,
    PartialResult,
    BrowserUnavailable,
}

#[derive(Debug, Clone, Serialize)]
pub struct SwitchResult {
    pub kind: SwitchResultKind,
    pub message: String,
}

impl SwitchResult {
    fn new(kind: SwitchResultKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub fn settings_uri(browser: &Browser) -> String {
    let encoded = browser.registration_name.replace(' ', "%20");
    format!("ms-settings:defaultapps?registeredAppMachine={encoded}")
}

pub fn open_supported(browser: &Browser) -> Result<SwitchResult> {
    if !browser.installed {
        return Ok(SwitchResult::new(
            SwitchResultKind::BrowserUnavailable,
            format!(
                "{} is not registered for all web-link associations.",
                browser.display_name
            ),
        ));
    }
    platform::open_uri(&settings_uri(browser))?;
    Ok(SwitchResult::new(
        SwitchResultKind::ConfirmationRequired,
        format!(
            "Windows Default Apps opened for {}. Click Set default to confirm.",
            browser.display_name
        ),
    ))
}

pub fn switch(
    snapshot: &BrowserSnapshot,
    target: BrowserId,
    mode: SwitchMode,
    warning_acknowledged: bool,
) -> Result<SwitchResult> {
    let Some(browser) = snapshot.browser(target) else {
        return Ok(SwitchResult::new(
            SwitchResultKind::BrowserUnavailable,
            "The selected browser is unavailable.",
        ));
    };
    if !browser.installed {
        return Ok(SwitchResult::new(
            SwitchResultKind::BrowserUnavailable,
            format!(
                "{} is not correctly registered with Windows.",
                browser.display_name
            ),
        ));
    }
    match mode {
        SwitchMode::Supported => open_supported(browser),
        SwitchMode::Experimental if !warning_acknowledged => Ok(SwitchResult::new(
            SwitchResultKind::UnsupportedWindows,
            "Experimental mode must be acknowledged in the tray menu before it can write protected associations.",
        )),
        SwitchMode::Experimental => {
            let result = experimental::apply(browser)?;
            if result.kind == SwitchResultKind::Changed { Ok(result) } else {
                let _ = platform::open_uri(&settings_uri(browser));
                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn settings_link_escapes_browser_name() {
        let browser = Browser {
            id: BrowserId::Edge,
            display_name: "Microsoft Edge".into(),
            registration_name: "Microsoft Edge".into(),
            executable: None,
            icon_source: None,
            prog_ids: BTreeMap::new(),
            installed: true,
        };
        assert_eq!(
            settings_uri(&browser),
            "ms-settings:defaultapps?registeredAppMachine=Microsoft%20Edge"
        );
    }
}
