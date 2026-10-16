use crate::associations;
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserId {
    Chrome,
    Edge,
}

impl BrowserId {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "chrome" => Some(Self::Chrome),
            "edge" => Some(Self::Edge),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Edge => "edge",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Browser {
    pub id: BrowserId,
    pub display_name: String,
    pub registration_name: String,
    pub executable: Option<PathBuf>,
    pub icon_source: Option<String>,
    pub prog_ids: BTreeMap<String, String>,
    pub installed: bool,
}

impl Browser {
    pub fn prog_id_for(&self, association: &str) -> Option<&str> {
        self.prog_ids.get(association).map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CurrentBrowser {
    Chrome,
    Edge,
    Mixed,
    Other,
    Unknown,
}

impl CurrentBrowser {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Chrome => "Google Chrome",
            Self::Edge => "Microsoft Edge",
            Self::Mixed => "Mixed web defaults",
            Self::Other => "Another browser",
            Self::Unknown => "Not configured",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserSnapshot {
    pub browsers: Vec<Browser>,
    pub current: CurrentBrowser,
    pub associations: BTreeMap<String, Option<String>>,
}

impl BrowserSnapshot {
    pub fn discover() -> Self {
        let browsers = vec![
            discover_browser(BrowserId::Chrome),
            discover_browser(BrowserId::Edge),
        ];
        let associations = associations::read_web_defaults();
        let current = classify_current(&browsers, &associations);
        Self {
            browsers,
            current,
            associations,
        }
    }

    pub fn browser(&self, id: BrowserId) -> Option<&Browser> {
        self.browsers.iter().find(|b| b.id == id)
    }

    pub fn target(&self) -> Option<BrowserId> {
        match self.current {
            CurrentBrowser::Chrome => Some(BrowserId::Edge),
            CurrentBrowser::Edge => Some(BrowserId::Chrome),
            _ => {
                if self.browser(BrowserId::Chrome).is_some_and(|b| b.installed) {
                    Some(BrowserId::Chrome)
                } else if self.browser(BrowserId::Edge).is_some_and(|b| b.installed) {
                    Some(BrowserId::Edge)
                } else {
                    None
                }
            }
        }
    }
}

fn classify_current(
    browsers: &[Browser],
    actual: &BTreeMap<String, Option<String>>,
) -> CurrentBrowser {
    let values: Vec<&str> = associations::WEB_ASSOCIATIONS
        .iter()
        .filter_map(|a| actual.get(a.name()).and_then(|v| v.as_deref()))
        .collect();
    if values.is_empty() {
        return CurrentBrowser::Unknown;
    }
    for browser in browsers.iter().filter(|b| b.installed) {
        let all = associations::WEB_ASSOCIATIONS.iter().all(|association| {
            actual
                .get(association.name())
                .and_then(|v| v.as_deref())
                .zip(browser.prog_id_for(association.name()))
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        });
        if all {
            return match browser.id {
                BrowserId::Chrome => CurrentBrowser::Chrome,
                BrowserId::Edge => CurrentBrowser::Edge,
            };
        }
    }
    if values
        .windows(2)
        .any(|pair| !pair[0].eq_ignore_ascii_case(pair[1]))
    {
        CurrentBrowser::Mixed
    } else {
        CurrentBrowser::Other
    }
}

#[cfg(windows)]
fn discover_browser(id: BrowserId) -> Browser {
    use winreg::{enums::*, RegKey};

    let expected = match id {
        BrowserId::Chrome => ["Google Chrome", "Google Chrome Beta", "Google Chrome SxS"],
        BrowserId::Edge => [
            "Microsoft Edge",
            "Microsoft Edge Beta",
            "Microsoft Edge Dev",
        ],
    };
    let hives = [
        (RegKey::predef(HKEY_CURRENT_USER), "HKCU"),
        (RegKey::predef(HKEY_LOCAL_MACHINE), "HKLM"),
    ];
    for (hive, _) in hives {
        if let Ok(registered) = hive.open_subkey("Software\\RegisteredApplications") {
            for name in expected {
                let Ok(cap_path): Result<String, _> = registered.get_value(name) else {
                    continue;
                };
                let Ok(capabilities) = hive.open_subkey(&cap_path) else {
                    continue;
                };
                let display_name = capabilities
                    .get_value("ApplicationName")
                    .unwrap_or_else(|_| name.to_string());
                let icon_source = capabilities.get_value("ApplicationIcon").ok();
                let mut prog_ids = BTreeMap::new();
                if let Ok(urls) = capabilities.open_subkey("URLAssociations") {
                    for key in ["http", "https"] {
                        if let Ok(value) = urls.get_value::<String, _>(key) {
                            prog_ids.insert(key.to_string(), value);
                        }
                    }
                }
                if let Ok(files) = capabilities.open_subkey("FileAssociations") {
                    for key in [".htm", ".html"] {
                        if let Ok(value) = files.get_value::<String, _>(key) {
                            prog_ids.insert(key.to_string(), value);
                        }
                    }
                }
                let executable = capabilities
                    .get_value::<String, _>("ApplicationIcon")
                    .ok()
                    .and_then(|s| associations::icon_executable(&s));
                let installed = associations::WEB_ASSOCIATIONS
                    .iter()
                    .all(|a| prog_ids.contains_key(a.name()));
                return Browser {
                    id,
                    display_name,
                    registration_name: name.to_string(),
                    executable,
                    icon_source,
                    prog_ids,
                    installed,
                };
            }
        }
    }
    let (display_name, registration_name) = match id {
        BrowserId::Chrome => ("Google Chrome", "Google Chrome"),
        BrowserId::Edge => ("Microsoft Edge", "Microsoft Edge"),
    };
    Browser {
        id,
        display_name: display_name.into(),
        registration_name: registration_name.into(),
        executable: None,
        icon_source: None,
        prog_ids: BTreeMap::new(),
        installed: false,
    }
}

#[cfg(not(windows))]
fn discover_browser(id: BrowserId) -> Browser {
    Browser {
        id,
        display_name: id.as_str().into(),
        registration_name: id.as_str().into(),
        executable: None,
        icon_source: None,
        prog_ids: BTreeMap::new(),
        installed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser(id: BrowserId, prefix: &str) -> Browser {
        let prog_ids = associations::WEB_ASSOCIATIONS
            .iter()
            .map(|a| (a.name().to_string(), format!("{prefix}HTML")))
            .collect();
        Browser {
            id,
            display_name: prefix.into(),
            registration_name: prefix.into(),
            executable: None,
            icon_source: None,
            prog_ids,
            installed: true,
        }
    }

    #[test]
    fn classifies_full_and_mixed_sets() {
        let browsers = vec![
            browser(BrowserId::Chrome, "Chrome"),
            browser(BrowserId::Edge, "Edge"),
        ];
        let chrome = associations::WEB_ASSOCIATIONS
            .iter()
            .map(|a| (a.name().into(), Some("ChromeHTML".into())))
            .collect();
        assert_eq!(classify_current(&browsers, &chrome), CurrentBrowser::Chrome);
        let mut mixed = chrome;
        mixed.insert("https".into(), Some("EdgeHTML".into()));
        assert_eq!(classify_current(&browsers, &mixed), CurrentBrowser::Mixed);
    }
}
