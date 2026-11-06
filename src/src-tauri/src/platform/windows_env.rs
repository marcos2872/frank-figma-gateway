//! Windows HKCU\Environment store.

use super::{EnvStore, PlatformError};
use ffg_core::env_plan::{EnvMutation, EnvSnapshot, ALL_PROXY, HTTPS_PROXY, HTTP_PROXY, NO_PROXY};
use std::ptr;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

const ENVIRONMENT_KEY: &str = "Environment";

#[derive(Debug, Default)]
pub struct WindowsEnvStore;

impl EnvStore for WindowsEnvStore {
    fn snapshot(&self) -> Result<EnvSnapshot, PlatformError> {
        with_key(|key| {
            Ok(EnvSnapshot {
                https_proxy: read_value(key, HTTPS_PROXY)?,
                http_proxy: read_value(key, HTTP_PROXY)?,
                no_proxy: read_value(key, NO_PROXY)?,
                all_proxy: read_value(key, ALL_PROXY)?,
            })
        })
    }

    fn apply(&self, mutations: &[EnvMutation]) -> Result<(), PlatformError> {
        with_key(|key| {
            for mutation in mutations {
                match mutation {
                    EnvMutation::Set { name, value } => write_value(key, name, value)?,
                    EnvMutation::Remove { name } => delete_value(key, name)?,
                }
            }
            Ok(())
        })?;
        broadcast_environment_change();
        Ok(())
    }
}

fn with_key<T>(action: impl FnOnce(HKEY) -> Result<T, PlatformError>) -> Result<T, PlatformError> {
    let mut key: HKEY = std::ptr::null_mut();
    let name: Vec<u16> = wide(ENVIRONMENT_KEY);
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

fn read_value(key: HKEY, name: &str) -> Result<Option<String>, PlatformError> {
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
    if !matches!(value_type, REG_SZ | REG_EXPAND_SZ) {
        return Ok(None);
    }
    let words: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|word| *word != 0)
        .collect();
    Ok(Some(String::from_utf16_lossy(&words)))
}

fn write_value(key: HKEY, name: &str, value: &str) -> Result<(), PlatformError> {
    let name = wide(name);
    let mut data: Vec<u16> = value.encode_utf16().collect();
    data.push(0);
    let bytes = data.len() * 2;
    let status = unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr() as *const u8,
            bytes as u32,
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

fn broadcast_environment_change() {
    let parameter = wide("Environment");
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
