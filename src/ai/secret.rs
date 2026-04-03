use std::process::Command;

use keyring::{Entry, Error as KeyringError};

use crate::ai::chat::ProviderKind;

const SERVICE_NAME: &str = "tinfo.ai";

pub trait SecretStore {
    fn load_provider_key(&self, provider: ProviderKind) -> Result<Option<String>, String>;
    fn save_provider_key(&self, provider: ProviderKind, api_key: &str) -> Result<(), String>;
}

#[derive(Default, Clone, Copy)]
pub struct SystemSecretStore;

impl SecretStore for SystemSecretStore {
    fn load_provider_key(&self, provider: ProviderKind) -> Result<Option<String>, String> {
        let entry = provider_entry(provider)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => load_provider_key_fallback(provider),
            Err(err) => {
                if let Some(value) = load_provider_key_fallback(provider)? {
                    Ok(Some(value))
                } else {
                    Err(format!(
                        "Failed to read {} API key from secure storage: {err}",
                        provider.label()
                    ))
                }
            }
        }
    }

    fn save_provider_key(&self, provider: ProviderKind, api_key: &str) -> Result<(), String> {
        let entry = provider_entry(provider)?;
        match entry.set_password(api_key) {
            Ok(()) => {}
            Err(err) => save_provider_key_fallback(provider, api_key).map_err(|fallback_err| {
                format!(
                    "Failed to save {} API key to secure storage: {err}; fallback failed: {fallback_err}",
                    provider.label()
                )
            })?,
        }

        if self.load_provider_key(provider)?.is_some() {
            Ok(())
        } else if save_provider_key_fallback(provider, api_key).is_ok()
            && self.load_provider_key(provider)?.is_some()
        {
            Ok(())
        } else {
            Err(format!(
                "Saved {} API key, but secure storage could not read it back.",
                provider.display_name()
            ))
        }
    }
}

pub fn remove_provider_key(provider: ProviderKind) -> Result<(), String> {
    let entry = provider_entry(provider)?;
    let _ = entry.delete_credential();

    let legacy_entry = legacy_provider_entry(provider)?;
    let _ = legacy_entry.delete_credential();

    #[cfg(target_os = "macos")]
    {
        remove_macos_keychain(provider)?;
    }

    Ok(())
}

fn load_provider_key_fallback(provider: ProviderKind) -> Result<Option<String>, String> {
    if let Some(value) = load_legacy_provider_key(provider)? {
        return Ok(Some(value));
    }

    #[cfg(target_os = "macos")]
    {
        return load_macos_keychain(provider);
    }

    #[allow(unreachable_code)]
    Ok(None)
}

fn save_provider_key_fallback(provider: ProviderKind, api_key: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return save_macos_keychain(provider, api_key);
    }

    #[allow(unreachable_code)]
    Err(format!(
        "{} secure storage fallback is not available on this platform",
        provider.display_name()
    ))
}

fn provider_entry(provider: ProviderKind) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, provider.secret_key_name())
        .map_err(|err| format!("Failed to initialize secure storage entry: {err}"))
}

fn legacy_provider_entry(provider: ProviderKind) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, legacy_secret_key_name(provider))
        .map_err(|err| format!("Failed to initialize legacy secure storage entry: {err}"))
}

fn load_legacy_provider_key(provider: ProviderKind) -> Result<Option<String>, String> {
    let entry = legacy_provider_entry(provider)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(format!(
            "Failed to read {} API key from legacy secure storage: {err}",
            provider.label()
        )),
    }
}

fn legacy_secret_key_name(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::OpenAi => "openai",
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::OpenRouter => "openrouter",
    }
}

#[cfg(target_os = "macos")]
fn load_macos_keychain(provider: ProviderKind) -> Result<Option<String>, String> {
    let account = provider.secret_key_name();
    let output = Command::new("security")
        .args(["find-generic-password", "-s", SERVICE_NAME, "-a", account, "-w"])
        .output()
        .map_err(|err| format!("Failed to run macOS keychain lookup: {err}"))?;

    if output.status.success() {
        let value = String::from_utf8(output.stdout)
            .map_err(|err| format!("Keychain returned invalid UTF-8: {err}"))?;
        return Ok(Some(value.trim_end_matches(['\r', '\n']).to_string()));
    }

    let stderr = String::from_utf8(output.stderr).unwrap_or_default();
    if stderr.contains("could not be found") || stderr.contains("The specified item could not be found") {
        return Ok(None);
    }

    Err(format!(
        "macOS Keychain lookup failed for {}: {}",
        provider.display_name(),
        stderr.trim()
    ))
}

#[cfg(target_os = "macos")]
fn save_macos_keychain(provider: ProviderKind, api_key: &str) -> Result<(), String> {
    let account = provider.secret_key_name();
    let _ = Command::new("security")
        .args(["delete-generic-password", "-s", SERVICE_NAME, "-a", account])
        .output();

    let output = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            SERVICE_NAME,
            "-a",
            account,
            "-w",
            api_key,
        ])
        .output()
        .map_err(|err| format!("Failed to run macOS keychain save: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8(output.stderr)
            .unwrap_or_else(|_| "macOS Keychain save failed".to_string()))
    }
}

#[cfg(target_os = "macos")]
fn remove_macos_keychain(provider: ProviderKind) -> Result<(), String> {
    for account in [provider.secret_key_name(), legacy_secret_key_name(provider)] {
        let output = Command::new("security")
            .args(["delete-generic-password", "-s", SERVICE_NAME, "-a", account])
            .output()
            .map_err(|err| format!("Failed to run macOS keychain delete: {err}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8(output.stderr).unwrap_or_default();
            if !stderr.contains("could not be found")
                && !stderr.contains("The specified item could not be found")
            {
                return Err(format!(
                    "Failed to delete {} keychain entry: {}",
                    provider.display_name(),
                    stderr.trim()
                ));
            }
        }
    }

    Ok(())
}
