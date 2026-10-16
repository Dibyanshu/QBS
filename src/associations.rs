use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationKind {
    Protocol,
    Extension,
}

#[derive(Debug, Clone, Copy)]
pub struct WebAssociation {
    name: &'static str,
    kind: AssociationKind,
}

impl WebAssociation {
    pub const fn new(name: &'static str, kind: AssociationKind) -> Self {
        Self { name, kind }
    }
    pub const fn name(self) -> &'static str {
        self.name
    }
    pub const fn kind(self) -> AssociationKind {
        self.kind
    }
}

pub const WEB_ASSOCIATIONS: [WebAssociation; 4] = [
    WebAssociation::new("http", AssociationKind::Protocol),
    WebAssociation::new("https", AssociationKind::Protocol),
    WebAssociation::new(".htm", AssociationKind::Extension),
    WebAssociation::new(".html", AssociationKind::Extension),
];

pub fn command_executable(command: &str) -> Option<PathBuf> {
    let trimmed = command.trim();
    let value = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        trimmed.split_whitespace().next()?
    };
    (!value.is_empty()).then(|| PathBuf::from(value))
}

pub fn icon_executable(icon: &str) -> Option<PathBuf> {
    let without_index = icon.rsplit_once(',').map_or(icon, |(path, _)| path);
    let value = without_index.trim().trim_matches('"');
    (!value.is_empty()).then(|| PathBuf::from(value))
}

pub fn user_choice_path(association: WebAssociation) -> String {
    match association.kind() {
        AssociationKind::Protocol => format!(
            "Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\{}\\UserChoice",
            association.name()
        ),
        AssociationKind::Extension => format!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{}\\UserChoice",
            association.name()
        ),
    }
}

#[cfg(windows)]
pub fn read_default(association: WebAssociation) -> Option<String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(user_choice_path(association))
        .ok()?
        .get_value("ProgId")
        .ok()
}

#[cfg(not(windows))]
pub fn read_default(_: WebAssociation) -> Option<String> {
    None
}

pub fn read_web_defaults() -> BTreeMap<String, Option<String>> {
    WEB_ASSOCIATIONS
        .iter()
        .map(|a| (a.name().to_string(), read_default(*a)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_quoted_and_plain_commands() {
        assert_eq!(
            command_executable(r#""C:\Program Files\Browser\browser.exe" --arg"#).unwrap(),
            PathBuf::from(r"C:\Program Files\Browser\browser.exe")
        );
        assert_eq!(
            command_executable(r"C:\Browser\browser.exe --arg").unwrap(),
            PathBuf::from(r"C:\Browser\browser.exe")
        );
        assert_eq!(
            icon_executable(r"C:\Program Files\Browser\browser.exe,0").unwrap(),
            PathBuf::from(r"C:\Program Files\Browser\browser.exe")
        );
    }
}
