//! Experimental legacy UserChoice hash backend.
//!
//! The hash algorithm is an independent Rust port of the MIT-licensed PS-SFTA
//! implementation by DanysysTeam (https://github.com/DanysysTeam/PS-SFTA).
//! Windows does not document or support this mechanism. Every write is followed
//! by a fresh registry read and QBS never reports success from the write alone.

use super::{SwitchResult, SwitchResultKind};
use crate::{
    associations::{self, WebAssociation},
    browser::Browser,
    platform,
};
use anyhow::{Context, Result};
use base64::Engine;
use chrono::{Timelike, Utc};
use md5::{Digest, Md5};

const EXPERIENCE_FALLBACK: &str =
    "User Choice set via Windows User Experience {D18B6DD5-6124-4341-9318-804003BAFA0B}";

pub(super) fn apply(browser: &Browser) -> Result<SwitchResult> {
    #[cfg(not(windows))]
    return Ok(SwitchResult::new(
        SwitchResultKind::UnsupportedWindows,
        "Experimental switching is available on Windows only.",
    ));

    #[cfg(windows)]
    {
        if user_choice_latest_present() {
            return Ok(SwitchResult::new(SwitchResultKind::UnsupportedWindows, "This Windows build uses UserChoiceLatest protection, which the experimental backend deliberately does not bypass. Windows Settings has been opened instead."));
        }
        let sid = platform::current_user_sid()?;
        let experience = user_experience_string();
        let timestamp = minute_filetime_hex();
        let mut write_errors = Vec::new();
        for association in associations::WEB_ASSOCIATIONS {
            let Some(prog_id) = browser.prog_id_for(association.name()) else {
                write_errors.push(association.name().to_string());
                continue;
            };
            let base = format!(
                "{}{}{}{}{}",
                association.name(),
                sid,
                prog_id,
                timestamp,
                experience
            )
            .to_lowercase();
            let hash = user_choice_hash(&base);
            if write_user_choice(association, prog_id, &hash).is_err() {
                write_errors.push(association.name().to_string());
            }
        }
        platform::notify_association_changed();
        std::thread::sleep(std::time::Duration::from_millis(350));
        let verified = associations::WEB_ASSOCIATIONS
            .iter()
            .filter(|association| {
                associations::read_default(**association)
                    .zip(browser.prog_id_for(association.name()).map(str::to_string))
                    .is_some_and(|(actual, wanted)| actual.eq_ignore_ascii_case(&wanted))
            })
            .count();
        if verified == associations::WEB_ASSOCIATIONS.len() {
            return Ok(SwitchResult::new(
                SwitchResultKind::Changed,
                format!(
                    "Default web browser changed to {} and all four associations verified.",
                    browser.display_name
                ),
            ));
        }
        let kind = if verified > 0 {
            SwitchResultKind::PartialResult
        } else {
            SwitchResultKind::VerificationFailed
        };
        let detail = if write_errors.is_empty() {
            format!("Windows accepted {verified}/4 associations")
        } else {
            format!(
                "writes failed for {}; {verified}/4 associations verified",
                write_errors.join(", ")
            )
        };
        Ok(SwitchResult::new(kind, format!("Experimental switching was not fully accepted ({detail}). Windows Settings has been opened for safe completion.")))
    }
}

#[cfg(windows)]
fn user_choice_latest_present() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    associations::WEB_ASSOCIATIONS.iter().any(|association| {
        let parent = associations::user_choice_path(*association)
            .trim_end_matches("\\UserChoice")
            .to_string();
        hkcu.open_subkey(parent)
            .ok()
            .and_then(|key| key.open_subkey("UserChoiceLatest").ok())
            .is_some()
    })
}

#[cfg(windows)]
fn write_user_choice(association: WebAssociation, prog_id: &str, hash: &str) -> Result<()> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = associations::user_choice_path(association);
    let parent_path = path.trim_end_matches("\\UserChoice");
    if let Ok(parent) = hkcu.open_subkey_with_flags(parent_path, winreg::enums::KEY_ALL_ACCESS) {
        let _ = parent.delete_subkey_all("UserChoice");
    }
    let (key, _) = hkcu
        .create_subkey(&path)
        .with_context(|| format!("create UserChoice for {}", association.name()))?;
    key.set_value("Hash", &hash)?;
    key.set_value("ProgId", &prog_id)?;
    Ok(())
}

fn minute_filetime_hex() -> String {
    let now = Utc::now()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();
    let filetime = (now.timestamp() + 11_644_473_600) as u64 * 10_000_000;
    format!("{filetime:016x}")
}

fn user_experience_string() -> String {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let path = std::path::PathBuf::from(windir)
        .join("System32")
        .join("Shell32.dll");
    let Ok(bytes) = std::fs::read(path) else {
        return EXPERIENCE_FALLBACK.into();
    };
    let words: Vec<u16> = bytes[..bytes.len().min(5 * 1024 * 1024)]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(&words);
    let needle = "User Choice set via Windows User Experience";
    text.find(needle)
        .and_then(|start| {
            text[start..]
                .find('}')
                .map(|end| text[start..=start + end].to_string())
        })
        .unwrap_or_else(|| EXPERIENCE_FALLBACK.into())
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("hash input padded"),
    )
}

fn calc(value: i64) -> i32 {
    value as i32
}
fn shr16(value: i32) -> i32 {
    ((value as u32) >> 16) as i32
}
fn mul(a: i32, b: u32) -> i32 {
    calc((a as i64) * (b as i32 as i64))
}

pub(crate) fn user_choice_hash(base_info: &str) -> String {
    let mut data = base_info
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    data.extend_from_slice(&[0, 0]);
    // PS-SFTA processes complete 8-byte pairs only. The terminating UTF-16
    // NUL participates when it completes a pair; any shorter tail is ignored.
    let pairs = data.len() / 8;
    while data.len() % 8 != 0 {
        data.push(0);
    }
    let md5 = Md5::digest(&data);
    let md51a = i32_at(&md5, 0) | 1;
    let md52a = i32_at(&md5, 4) | 1;
    if pairs == 0 {
        return String::new();
    }

    let mut cache = 0i32;
    let mut out1 = 0i32;
    let mut out2 = 0i32;
    let k1 = md51a.wrapping_add(0x69FB0000u32 as i32);
    let k2 = md52a.wrapping_add(0x13DB0000u32 as i32);
    for i in 0..pairs {
        let offset = i * 8;
        let r0 = i32_at(&data, offset).wrapping_add(out1);
        let r1 = i32_at(&data, offset + 4);
        let r20 = calc(mul(r0, k1 as u32) as i64 - mul(shr16(r0), 0x10FA9605) as i64);
        let r21 = calc(mul(r20, 0x79F8A395) as i64 + mul(shr16(r20), 0x689B6B9F) as i64);
        let r3 = calc(mul(r21, 0xEA970001) as i64 - mul(shr16(r21), 0x3C101569) as i64);
        let r4 = r3.wrapping_add(r1);
        let r5 = cache.wrapping_add(r3);
        let r60 = calc(mul(r4, k2 as u32) as i64 - mul(shr16(r4), 0x3CE8EC25) as i64);
        let r61 = calc(mul(r60, 0x59C3AF2D) as i64 - mul(shr16(r60), 0x2232E0F1) as i64);
        out1 = calc(mul(r61, 0x1EC90001) as i64 + mul(shr16(r61), 0x35BD1EC9) as i64);
        out2 = r5.wrapping_add(out1);
        cache = out2;
    }
    let first = (out1, out2);

    cache = 0;
    out1 = 0;
    out2 = 0;
    for i in 0..pairs {
        let offset = i * 8;
        let r0 = i32_at(&data, offset).wrapping_add(out1);
        let r10 = mul(r0, md51a as u32);
        let r11 = calc(mul(r10, 0xB1110000) as i64 - mul(shr16(r10), 0x30674EEF) as i64);
        let r20 = calc(mul(r11, 0x5B9F0000) as i64 - mul(shr16(r11), 0x78F7A461) as i64);
        let r21 = calc(mul(shr16(r20), 0x12CEB96D) as i64 - mul(r20, 0x46930000) as i64);
        let r3 = calc(mul(r21, 0x1D830000) as i64 + mul(shr16(r21), 0x257E1D83) as i64);
        let r40 = mul(r3.wrapping_add(i32_at(&data, offset + 4)), md52a as u32);
        let r41 = calc(mul(r40, 0x16F50000) as i64 - mul(shr16(r40), 0x5D8BE90B) as i64);
        let r50 = calc(mul(r41, 0x96FF0000) as i64 - mul(shr16(r41), 0x2C7C6901) as i64);
        let r51 = calc(mul(r50, 0x2B890000) as i64 + mul(shr16(r50), 0x7C932B89) as i64);
        out1 = calc(mul(r51, 0x9F690000) as i64 - mul(shr16(r51), 0x405B6097) as i64);
        out2 = out1.wrapping_add(cache).wrapping_add(r3);
        cache = out2;
    }
    let mut result = Vec::with_capacity(8);
    result.extend_from_slice(&(out1 ^ first.0).to_le_bytes());
    result.extend_from_slice(&(out2 ^ first.1).to_le_bytes());
    base64::engine::general_purpose::STANDARD.encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_is_stable_and_base64_sized() {
        let input = ".htmls-1-5-21-1-2-3-1001chromehtml01db000000000000user choice set via windows user experience {d18b6dd5-6124-4341-9318-804003ba fa0b}".replace(' ', "");
        let hash = user_choice_hash(&input);
        assert_eq!(hash.len(), 12);
        assert_eq!(hash, user_choice_hash(&input));
    }
    #[test]
    fn filetime_is_sixteen_lower_hex_digits() {
        let value = minute_filetime_hex();
        assert_eq!(value.len(), 16);
        assert!(value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
