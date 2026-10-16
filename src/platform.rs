use anyhow::{Context, Result};
use std::path::PathBuf;

#[cfg(windows)]
pub fn current_user_sid() -> Result<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HLOCAL};
    use windows::Win32::Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = Default::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        let mut buffer = vec![0u8; needed as usize];
        GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut needed,
        )?;
        let token_user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(token_user.User.Sid, &mut text)?;
        let sid = text.to_string()?;
        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(text.0.cast())));
        let _ = CloseHandle(token);
        Ok(sid.to_ascii_lowercase())
    }
}

#[cfg(not(windows))]
pub fn current_user_sid() -> Result<String> {
    anyhow::bail!("QBS supports Windows only")
}

pub fn executable_path() -> Result<PathBuf> {
    std::env::current_exe().context("cannot resolve QBS executable path")
}

#[cfg(windows)]
pub fn style_widget_window(hwnd: isize, opacity: f32) -> Result<()> {
    let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA,
        WS_EX_LAYERED,
    };

    unsafe {
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED.0 as i32);
        SetLayeredWindowAttributes(
            hwnd,
            Default::default(),
            (opacity.clamp(0.35, 1.0) * 255.0).round() as u8,
            LWA_ALPHA,
        )?;
        let preference = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        )?;
    }
    Ok(())
}

#[cfg(windows)]
pub fn snap_window(
    hwnd: isize,
    position: (i32, i32),
    size: (i32, i32),
    threshold: i32,
) -> (i32, i32) {
    let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let area = info.rcWork;
            let right = area.right - size.0;
            let bottom = area.bottom - size.1;
            let x = if (position.0 - area.left).abs() <= threshold {
                area.left
            } else if (position.0 - right).abs() <= threshold {
                right
            } else {
                position.0
            };
            let y = if (position.1 - area.top).abs() <= threshold {
                area.top
            } else if (position.1 - bottom).abs() <= threshold {
                bottom
            } else {
                position.1
            };
            return (x, y);
        }
    }
    position
}

#[cfg(windows)]
pub fn show_message(title: &str, body: &str) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
    let title: Vec<u16> = std::ffi::OsStr::new(title)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let body: Vec<u16> = std::ffi::OsStr::new(body)
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

#[cfg(windows)]
pub fn set_start_with_windows(enabled: bool) -> Result<()> {
    use winreg::{enums::*, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run, _) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;
    if enabled {
        run.set_value(
            "QuickBrowserSwitcher",
            &format!("\"{}\"", executable_path()?.display()),
        )?;
    } else {
        match run.delete_value("QuickBrowserSwitcher") {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_start_with_windows(_: bool) -> Result<()> {
    anyhow::bail!("QBS supports Windows only")
}

#[cfg(windows)]
pub fn open_uri(uri: &str) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide: Vec<u16> = std::ffi::OsStr::new(uri)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result =
        unsafe { ShellExecuteW(None, None, PCWSTR(wide.as_ptr()), None, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 {
        anyhow::bail!("Windows could not open {uri}")
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn open_uri(_: &str) -> Result<()> {
    anyhow::bail!("QBS supports Windows only")
}

#[cfg(windows)]
pub fn notify_association_changed() {
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

#[cfg(not(windows))]
pub fn notify_association_changed() {}
