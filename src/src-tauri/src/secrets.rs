//! Platform secret store: Credential Manager on Windows, memory elsewhere.

use ffg_core::secrets::{SecretError, SecretKey, SecretStore, SecretValue};

#[cfg(not(windows))]
pub type PlatformSecretStore = ffg_core::secrets::MemorySecretStore;

#[cfg(windows)]
pub use keyring_store::KeyringStore as PlatformSecretStore;

#[cfg(windows)]
mod keyring_store {
    use super::*;
    use keyring::Entry;

    #[derive(Debug, Clone)]
    pub struct KeyringStore {
        service: String,
    }

    impl Default for KeyringStore {
        fn default() -> Self {
            Self {
                service: "frank-figma-gateway".to_owned(),
            }
        }
    }

    impl KeyringStore {
        fn entry(&self, key: &SecretKey) -> Result<Entry, SecretError> {
            Entry::new(&self.service, key.as_str()).map_err(|error| SecretError(error.to_string()))
        }
    }

    impl SecretStore for KeyringStore {
        fn get(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretError> {
            match self.entry(key)?.get_password() {
                Ok(value) => Ok(Some(SecretValue::new(value))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(SecretError(error.to_string())),
            }
        }

        fn set(&self, key: &SecretKey, value: &SecretValue) -> Result<(), SecretError> {
            self.entry(key)?
                .set_password(value.expose())
                .map_err(|error| SecretError(error.to_string()))
        }

        fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
            match self.entry(key)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(SecretError(error.to_string())),
            }
        }
    }
}

pub const MASTER_KEY: &str = "ffg/litellm/master-key";
pub const SALT_KEY: &str = "ffg/litellm/salt-key";

pub fn ensure_secret(
    store: &dyn SecretStore,
    key: &'static str,
) -> Result<SecretValue, SecretError> {
    let key = SecretKey::new(key);
    if let Some(value) = store.get(&key)? {
        return Ok(value);
    }
    let value = SecretValue::new(format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    ));
    store.set(&key, &value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reuses_a_secret() {
        let store = ffg_core::secrets::MemorySecretStore::default();
        let first = ensure_secret(&store, "ffg/test/key").expect("first");
        let second = ensure_secret(&store, "ffg/test/key").expect("second");
        assert_eq!(first, second);
        assert!(!first.expose().is_empty());
    }
}
