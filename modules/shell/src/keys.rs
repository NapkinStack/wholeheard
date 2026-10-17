//! A provider's key, kept in the system's keyring — the Secret Service under a Linux desktop, the
//! Keychain on macOS, the Credential Manager on Windows — under Summit's name and the provider's
//! host. Never in a file, never in what Summit says.

use keyring_core::{Entry, Error};

/// What Summit's keys are filed under, in the keyring.
const SERVICE: &str = "dev.summit.Summit";

/// Opens the system's keyring for this process, once, before a key is read or kept. Refused —
/// no keyring service on this desktop — the sentence says why.
pub fn open_system_keyring() -> Result<(), String> {
    #[cfg(all(unix, not(target_os = "macos")))]
    let store = zbus_secret_service_keyring_store::Store::new();
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new();
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new();
    keyring_core::set_default_store(store.map_err(|error| error.to_string())?);
    Ok(())
}

fn entry(host: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, host).map_err(|error| error.to_string())
}

/// Keeps a provider's key, replacing the one kept for that host.
pub fn keep(host: &str, key: &str) -> Result<(), String> {
    entry(host)?.set_password(key).map_err(|error| error.to_string())
}

/// The key kept for a provider's host; none when none is.
pub fn key(host: &str) -> Result<Option<String>, String> {
    match entry(host)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Forgets the key kept for a provider's host; nothing kept is nothing to forget.
pub fn forget(host: &str) -> Result<(), String> {
    match entry(host)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}
