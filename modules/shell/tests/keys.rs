//! A provider's key in a keyring — the keyring library's own stand-in for the system's, so that
//! no test reads or writes the user's.

use summit_shell::keys;

#[test]
fn a_key_is_kept_by_its_host_replaced_and_forgotten() {
    keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
    assert_eq!(keys::key("openrouter.ai"), Ok(None));
    keys::keep("openrouter.ai", "sk-first").unwrap();
    keys::keep("api.mistral.ai", "mistral-key").unwrap();
    assert_eq!(keys::key("openrouter.ai").unwrap().as_deref(), Some("sk-first"));
    keys::keep("openrouter.ai", "sk-second").unwrap();
    assert_eq!(keys::key("openrouter.ai").unwrap().as_deref(), Some("sk-second"));
    // Each host its own key.
    assert_eq!(keys::key("api.mistral.ai").unwrap().as_deref(), Some("mistral-key"));
    keys::forget("openrouter.ai").unwrap();
    assert_eq!(keys::key("openrouter.ai"), Ok(None));
    // Forgetting what is not kept is nothing.
    assert_eq!(keys::forget("openrouter.ai"), Ok(()));

    // Filed under Summit's name and the host: a key kept by a version before is found again.
    keys::keep("api.openai.com", "sk-filed").unwrap();
    let filed = keyring_core::Entry::new("dev.summit.Summit", "api.openai.com").unwrap();
    assert_eq!(filed.get_password().unwrap(), "sk-filed");

    // A keyring that fails is not "no key": a locked one would otherwise ask for a key again.
    let mock = filed.as_any().downcast_ref::<keyring_core::mock::Cred>().unwrap();
    mock.set_error(keyring_core::Error::PlatformFailure("locked".into()));
    assert!(keys::key("api.openai.com").is_err());
}
