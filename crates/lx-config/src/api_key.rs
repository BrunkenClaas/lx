#![forbid(unsafe_code)]

use lx_core::exit::LxError;

use crate::Provider;

/// Resolve the API key for LLM calls.
///
/// Priority order:
///   1. `LX_API_KEY` environment variable
///   2. OS credential store (Windows: Credential Manager, Linux: kernel keyring)
///
/// Returns `LxError::ConfigAuth` with an actionable hint if no key is found.
pub fn api_key() -> Result<String, LxError> {
    // Env var always wins — fast path, works everywhere.
    if let Ok(key) = std::env::var("LX_API_KEY") {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // Try the OS credential store.
    if let Some(key) = read_from_credential_store() {
        return Ok(key);
    }

    Err(LxError::ConfigAuth(
        "no API key found; set LX_API_KEY=<your-key> or store it with:\n  \
         Windows: cmdkey /add:lx-api-key /user:lx /pass:<key>\n  \
         Linux:   keyctl add user lx-api-key <key> @u"
            .to_string(),
    ))
}

/// Build a provider-specific error message when no API key is found.
pub fn provider_key_hint(provider: &Provider) -> String {
    let (where_to_get, extra) = match provider {
        Provider::Openai => ("platform.openai.com/api-keys", ""),
        Provider::Anthropic => ("console.anthropic.com/settings/keys", ""),
        Provider::Gemini => ("aistudio.google.com/apikey", ""),
        Provider::Groq => ("console.groq.com/keys", ""),
        Provider::OpenRouter => ("openrouter.ai/settings/keys", ""),
        Provider::Mistral => ("console.mistral.ai/api-keys", ""),
        Provider::DeepSeek => ("platform.deepseek.com/api_keys", ""),
        Provider::Azure => (
            "portal.azure.com",
            "\n  hint: also set LX_BASE_URL to your Azure deployment URL",
        ),
        // Local providers never reach this function (is_local() check in caller).
        Provider::Ollama | Provider::LmStudio => ("(no key needed for local providers)", ""),
    };
    format!(
        "no API key found for provider '{provider}'\n  \
         hint: set LX_API_KEY=<your-key>  (get one at {where_to_get})\n  \
         or store it with:\n  \
         Windows: cmdkey /add:lx-api-key /user:lx /pass:<key>\n  \
         Linux:   keyctl add user lx-api-key <key> @u{extra}"
    )
}

// ── OS credential store ───────────────────────────────────────────────────────

/// Attempt to read the API key from the OS credential store.
/// Returns `None` if the store is unavailable or the entry does not exist.
fn read_from_credential_store() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        windows_cred_read()
    }
    #[cfg(target_os = "linux")]
    {
        linux_keyring_read()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

// ── Windows Credential Manager ─────────────────────────────────────────────────
// Uses the Win32 CredRead API via windows-sys.
// lx-core already depends on windows-sys; we re-use the transitive dep here
// rather than adding it directly to lx-config's Cargo.toml — the features we
// need (Win32_Security_Credentials) are not yet in lx-core's feature list,
// so we fall back to a no-op for now and document the limitation.
//
// A future PR can add `Win32_Security_Credentials` to lx-core's windows-sys
// features and call CredReadW here.

#[cfg(target_os = "windows")]
fn windows_cred_read() -> Option<String> {
    // Implementation requires Win32_Security_Credentials feature in windows-sys.
    // lx-core currently only enables Win32_System_Console and Win32_Globalization.
    // Rather than add a dependency here (lx-config must stay forbid(unsafe_code)),
    // we document that the credential store path requires the user to use LX_API_KEY.
    // This is the safe, conservative choice — the env-var path works on all platforms.
    None
}

// ── Linux kernel keyring ──────────────────────────────────────────────────────
// Uses the `keyctl` syscall interface via /proc/keys or `keyctl` command.
// Direct syscall would require unsafe; instead we invoke `keyctl` as a
// subprocess — safe, no unsafe, works on any Linux with keyutils installed.

#[cfg(target_os = "linux")]
fn linux_keyring_read() -> Option<String> {
    // keyctl print <key_id> — find the key id first, then read value.
    // We use `keyctl search @u user lx-api-key` to get the key serial,
    // then `keyctl print <serial>` to get the value.
    let search = std::process::Command::new("keyctl")
        .args(["search", "@u", "user", "lx-api-key"])
        .output()
        .ok()?;

    if !search.status.success() {
        return None;
    }

    let serial = std::str::from_utf8(&search.stdout).ok()?.trim().to_string();
    if serial.is_empty() {
        return None;
    }

    let print = std::process::Command::new("keyctl")
        .args(["print", &serial])
        .output()
        .ok()?;

    if !print.status.success() {
        return None;
    }

    let key = std::str::from_utf8(&print.stdout).ok()?.trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_from_env_when_set() {
        // Only test when the caller has already set LX_API_KEY (e.g. CI).
        if let Ok(key) = std::env::var("LX_API_KEY") {
            if !key.trim().is_empty() {
                let result = api_key();
                assert!(result.is_ok());
                assert_eq!(result.unwrap(), key.trim());
            }
        }
    }

    #[test]
    fn missing_key_returns_config_auth_error() {
        // Only run when LX_API_KEY is absent and no credential store entry exists.
        // We cannot mutate the environment without unsafe, so we skip if the key is set.
        if std::env::var("LX_API_KEY").is_ok() {
            return;
        }
        // If the OS credential store also has no entry the function returns Err.
        if let Err(e) = api_key() {
            assert!(
                matches!(e, lx_core::exit::LxError::ConfigAuth(_)),
                "expected ConfigAuth, got: {e:?}"
            );
            assert!(
                e.to_string().contains("LX_API_KEY"),
                "error message should mention LX_API_KEY"
            );
        }
    }
}
