//! WinINET system proxy (Internet Settings) for the current user.
//!
//! O renderer da Figma (Chromium) não lê `HTTPS_PROXY`; ele usa o proxy do
//! sistema. Sem isto, só o agente Node passaria pelo nosso proxy.

use super::{PlatformError, SystemProxySnapshot};
use std::ptr;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const PROXY_ENABLE: &str = "ProxyEnable";
const PROXY_SERVER: &str = "ProxyServer";
const PROXY_OVERRIDE: &str = "ProxyOverride";
const AUTO_CONFIG_URL: &str = "AutoConfigURL";

#[derive(Debug, Default)]
pub struct WindowsSystemProxy;

impl super::SystemProxyStore for WindowsSystemProxy {
    fn snapshot(&self) -> Result<SystemProxySnapshot, PlatformError> {
        with_key(|key| {
            Ok(SystemProxySnapshot {
                enabled: read_dword(key, PROXY_ENABLE)?,
                server: read_string(key, PROXY_SERVER)?,
                override_list: read_string(key, PROXY_OVERRIDE)?,
                auto_config_url: read_string(key, AUTO_CONFIG_URL)?,
            })
        })
    }

    fn apply(&self, port: u16) -> Result<(), PlatformError> {
        with_key(|key| {
            write_string(key, PROXY_SERVER, &format!("127.0.0.1:{port}"))?;
            write_dword(key, PROXY_ENABLE, 1)?;
            write_string(key, PROXY_OVERRIDE, "<local>;localhost;127.0.0.1;::1")?;
            // PAC tem precedência sobre o proxy fixo.
            delete_value(key, AUTO_CONFIG_URL)?;
            Ok(())
        })?;
        refresh();
        Ok(())
    }

    fn restore(&self, snapshot: &SystemProxySnapshot) -> Result<(), PlatformError> {
        with_key(|key| {
            match snapshot.enabled {
                Some(value) => write_dword(key, PROXY_ENABLE, value)?,
                None => delete_value(key, PROXY_ENABLE)?,
            }
            match &snapshot.server {
                Some(value) => write_string(key, PROXY_SERVER, value)?,
                None => delete_value(key, PROXY_SERVER)?,
            }
            match &snapshot.override_list {
                Some(value) => write_string(key, PROXY_OVERRIDE, value)?,
                None => delete_value(key, PROXY_OVERRIDE)?,
            }
            match &snapshot.auto_config_url {
                Some(value) => write_string(key, AUTO_CONFIG_URL, value)?,
                None => delete_value(key, AUTO_CONFIG_URL)?,
            }
            Ok(())
        })?;
        refresh();
        Ok(())
    }
}

fn with_key<T>(action: impl FnOnce(HKEY) -> Result<T, PlatformError>) -> Result<T, PlatformError> {
    let mut key: HKEY = ptr::null_mut();
    let name = wide(INTERNET_SETTINGS);
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            name.as_ptr(),
            0,
            KEY_READ | KEY_WRITE,
            &mut key,
        )
    };
    if status != 0 {
        return Err(PlatformError(format!("RegOpenKeyExW falhou ({status})")));
    }
    let result = action(key);
    unsafe {
        RegCloseKey(key);
    }
    result
}

fn read_dword(key: HKEY, name: &str) -> Result<Option<u32>, PlatformError> {
    let name = wide(name);
    let mut value_type = 0u32;
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut value_type,
            &mut value as *mut u32 as *mut u8,
            &mut size,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != 0 {
        return Err(PlatformError(format!("RegQueryValueExW falhou ({status})")));
    }
    Ok(Some(value))
}

fn read_string(key: HKEY, name: &str) -> Result<Option<String>, PlatformError> {
    let name = wide(name);
    let mut value_type = 0u32;
    let mut size = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut value_type,
            ptr::null_mut(),
            &mut size,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != 0 {
        return Err(PlatformError(format!("RegQueryValueExW falhou ({status})")));
    }
    if size == 0 {
        return Ok(Some(String::new()));
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            ptr::null(),
            &mut value_type,
            buffer.as_mut_ptr(),
            &mut size,
        )
    };
    if status != 0 {
        return Err(PlatformError(format!("RegQueryValueExW falhou ({status})")));
    }
    if value_type != REG_SZ {
        return Ok(None);
    }
    let words: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|word| *word != 0)
        .collect();
    Ok(Some(String::from_utf16_lossy(&words)))
}

fn write_string(key: HKEY, name: &str, value: &str) -> Result<(), PlatformError> {
    let name = wide(name);
    let mut data: Vec<u16> = value.encode_utf16().collect();
    data.push(0);
    let status = unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            (data.len() * 2) as u32,
        )
    };
    if status != 0 {
        return Err(PlatformError(format!("RegSetValueExW falhou ({status})")));
    }
    Ok(())
}

fn write_dword(key: HKEY, name: &str, value: u32) -> Result<(), PlatformError> {
    let name = wide(name);
    let status = unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_DWORD,
            &value as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        )
    };
    if status != 0 {
        return Err(PlatformError(format!("RegSetValueExW falhou ({status})")));
    }
    Ok(())
}

fn delete_value(key: HKEY, name: &str) -> Result<(), PlatformError> {
    let name = wide(name);
    let status = unsafe { RegDeleteValueW(key, name.as_ptr()) };
    if status == 0 || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(PlatformError(format!("RegDeleteValueW falhou ({status})")))
    }
}

fn refresh() {
    let parameter = wide("Internet Settings");
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            parameter.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5_000,
            &mut result,
        );
    }
}

fn wide(value: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    wide
}
