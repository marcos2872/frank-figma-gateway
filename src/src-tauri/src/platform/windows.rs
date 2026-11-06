//! Windows Credential Manager and DPAPI adapters.

use super::{CertStore, FileProtector, PlatformError};
use std::ptr;
use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
use windows_sys::Win32::Security::Cryptography::{
    CertAddEncodedCertificateToStore, CertCloseStore, CertDeleteCertificateFromStore,
    CertFindCertificateInStore, CertFreeCertificateContext, CertOpenStore, CryptProtectData,
    CryptUnprotectData, CERT_CONTEXT, CERT_FIND_HASH, CERT_STORE_ADD_REPLACE_EXISTING,
    CERT_STORE_PROV_SYSTEM_W, CERT_SYSTEM_STORE_CURRENT_USER, CRYPTPROTECT_UI_FORBIDDEN,
    CRYPT_INTEGER_BLOB, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
};

const STORE_NAME: &str = "ROOT";

pub struct WindowsCertStore;

impl Default for WindowsCertStore {
    fn default() -> Self {
        Self
    }
}

impl CertStore for WindowsCertStore {
    fn is_installed(&self, thumbprint: &str) -> Result<bool, PlatformError> {
        with_store(|store| {
            let context = find_by_thumbprint(store, thumbprint)?;
            if context.is_null() {
                return Ok(false);
            }
            unsafe {
                CertFreeCertificateContext(context);
            }
            Ok(true)
        })
    }

    fn install(&self, _thumbprint: &str, der: &[u8]) -> Result<(), PlatformError> {
        with_store(|store| {
            let ok = unsafe {
                CertAddEncodedCertificateToStore(
                    store,
                    X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                    der.as_ptr(),
                    der.len() as u32,
                    CERT_STORE_ADD_REPLACE_EXISTING,
                    ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(last_error("CertAddEncodedCertificateToStore"));
            }
            Ok(())
        })
    }

    fn remove(&self, thumbprint: &str) -> Result<bool, PlatformError> {
        with_store(|store| {
            let context = find_by_thumbprint(store, thumbprint)?;
            if context.is_null() {
                return Ok(false);
            }
            let ok = unsafe { CertDeleteCertificateFromStore(context) };
            if ok == 0 {
                return Err(last_error("CertDeleteCertificateFromStore"));
            }
            Ok(true)
        })
    }
}

fn with_store<T>(
    action: impl FnOnce(*mut core::ffi::c_void) -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    let mut name: Vec<u16> = STORE_NAME.encode_utf16().collect();
    name.push(0);
    let store = unsafe {
        CertOpenStore(
            CERT_STORE_PROV_SYSTEM_W,
            0,
            0,
            CERT_SYSTEM_STORE_CURRENT_USER,
            name.as_ptr() as *const core::ffi::c_void,
        )
    };
    if store.is_null() {
        return Err(last_error("CertOpenStore"));
    }
    let result = action(store);
    unsafe {
        CertCloseStore(store, 0);
    }
    result
}

fn find_by_thumbprint(
    store: *mut core::ffi::c_void,
    thumbprint: &str,
) -> Result<*mut CERT_CONTEXT, PlatformError> {
    let bytes = hex::decode(thumbprint)
        .map_err(|error| PlatformError(format!("thumbprint inválido: {error}")))?;
    let hash = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let context = unsafe {
        CertFindCertificateInStore(
            store,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            0,
            CERT_FIND_HASH,
            &hash as *const _ as *const core::ffi::c_void,
            ptr::null(),
        )
    };
    Ok(context)
}

pub struct WindowsDpapi;

impl Default for WindowsDpapi {
    fn default() -> Self {
        Self
    }
}

impl FileProtector for WindowsDpapi {
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(last_error("CryptProtectData"));
        }
        let result = unsafe { copy_and_free(output) };
        Ok(result)
    }

    fn unprotect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(last_error("CryptUnprotectData"));
        }
        let result = unsafe { copy_and_free(output) };
        Ok(result)
    }
}

unsafe fn copy_and_free(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let result = if blob.pbData.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec()
    };
    if !blob.pbData.is_null() {
        LocalFree(blob.pbData as *mut core::ffi::c_void);
    }
    result
}

fn last_error(call: &str) -> PlatformError {
    let code = unsafe { GetLastError() };
    PlatformError(format!("{call} falhou (Win32 {code})"))
}
