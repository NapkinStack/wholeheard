//! A provider's key, kept in the system's keyring — the Secret Service under a Linux desktop, the
//! Keychain on macOS, the Credential Manager on Windows — under Wholeheard's name and the provider's
//! host. Never in a file, never in what Wholeheard says.

use keyring_core::{Entry, Error};

/// What Wholeheard's keys are filed under, in the keyring.
const SERVICE: &str = "dev.wholeheard.Wholeheard";
/// What they were filed under before D28, when the product was named Summit.
const SERVICE_BEFORE: &str = "dev.summit.Summit";

/// Opens the system's keyring for this process, once, before a key is read or kept. Refused —
/// no keyring service on this desktop — the sentence says why. With `WHOLEHEARD_KEYRING=memory`, in
/// a build for trying Wholeheard only, a keyring that lives as long as the program instead: a scenario
/// never reads nor writes the user's, and a release never keeps a key where it does not last.
pub fn open_system_keyring() -> Result<(), String> {
    if cfg!(debug_assertions) && std::env::var("WHOLEHEARD_KEYRING").as_deref() == Ok("memory") {
        keyring_core::set_default_store(keyring_core::mock::Store::new().map_err(|error| error.to_string())?);
        return Ok(());
    }
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

/// The key kept for a provider's host; none when none is. One kept under Summit's name (before
/// D28) is moved under Wholeheard's as it is read: kept anew, then forgotten there.
pub fn key(host: &str) -> Result<Option<String>, String> {
    match entry(host)?.get_password() {
        Ok(key) => return Ok(Some(key)),
        Err(Error::NoEntry) => {}
        Err(error) => return Err(error.to_string()),
    }
    let before = Entry::new(SERVICE_BEFORE, host).map_err(|error| error.to_string())?;
    match before.get_password() {
        Ok(key) => {
            keep(host, &key)?;
            let _ = before.delete_credential();
            Ok(Some(key))
        }
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
