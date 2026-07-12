// platform.rs is the ONE place in the codebase that may use unsafe code and
// #[cfg(target_os)] guards. All other modules call the functions defined here.
//
// Windows-specific code uses `windows-sys` for console and locale APIs.
// Each unsafe block has a // SAFETY: comment.

use std::path::PathBuf;

// ── Config directory ──────────────────────────────────────────────────────────

/// Return the OS-appropriate user config directory for lx.
///
/// Linux/macOS: `$XDG_CONFIG_HOME/lx`  (fallback: `$HOME/.config/lx`)
/// Windows:     `%APPDATA%\lx`
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base).join("lx")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });
        base.join("lx")
    }
}

// ── TTY detection ─────────────────────────────────────────────────────────────

/// Numeric file descriptor used to identify a standard stream.
#[derive(Clone, Copy)]
pub enum Fd {
    Stdin = 0,
    Stdout = 1,
    Stderr = 2,
}

/// Return true if the given file descriptor is connected to a terminal.
pub fn is_tty(fd: Fd) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
        };

        use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
        let handle_id = match fd {
            Fd::Stdin => STD_INPUT_HANDLE,
            Fd::Stdout => STD_OUTPUT_HANDLE,
            Fd::Stderr => STD_ERROR_HANDLE,
        };
        // SAFETY: GetStdHandle is a documented Win32 API. We only read the
        // handle value; we do not close or transfer ownership.
        let handle = unsafe { GetStdHandle(handle_id) };
        if handle == INVALID_HANDLE_VALUE || handle == 0 as windows_sys::Win32::Foundation::HANDLE {
            return false;
        }
        // SAFETY: GetFileType with a valid console handle is safe. We do not
        // mutate any state.
        use windows_sys::Win32::System::Console::GetConsoleMode;
        let mut mode: u32 = 0;
        // SAFETY: GetConsoleMode writes into `mode` which we own.
        unsafe { GetConsoleMode(handle, &mut mode) != 0 }
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::io::IsTerminal;
        match fd {
            Fd::Stdin => std::io::stdin().is_terminal(),
            Fd::Stdout => std::io::stdout().is_terminal(),
            Fd::Stderr => std::io::stderr().is_terminal(),
        }
    }
}

// ── ANSI / colour enabling ────────────────────────────────────────────────────

/// Enable ANSI colour output on platforms that need explicit opt-in.
///
/// On Windows this:
///   1. Sets the console code page to UTF-8 (65001).
///   2. Enables `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on the stdout handle.
///
/// On other platforms this is a no-op (ANSI is on by default in terminals).
pub fn enable_ansi() {
    #[cfg(target_os = "windows")]
    {
        // Best-effort: ignore errors (e.g. when redirected to a file).
        let _ = try_enable_ansi_windows();
    }
    #[cfg(not(target_os = "windows"))]
    {
        // No-op on POSIX: ANSI escape sequences work without any opt-in.
    }
}

#[cfg(target_os = "windows")]
fn try_enable_ansi_windows() -> Result<(), ()> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
    };

    const UTF8_CODEPAGE: u32 = 65_001;

    // SAFETY: SetConsoleCP / SetConsoleOutputCP are documented Win32 APIs that
    // only affect the current process's console code page. No memory aliasing.
    unsafe {
        SetConsoleCP(UTF8_CODEPAGE);
        SetConsoleOutputCP(UTF8_CODEPAGE);
    }

    // SAFETY: GetStdHandle returns a pseudo-handle managed by the OS.
    // We do not close it.
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if handle == INVALID_HANDLE_VALUE || handle == 0 as windows_sys::Win32::Foundation::HANDLE {
        return Err(());
    }

    let mut mode: u32 = 0;
    // SAFETY: GetConsoleMode writes into caller-owned `mode`.
    if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
        return Err(());
    }

    // SAFETY: SetConsoleMode with a valid handle and valid flag is safe.
    if unsafe { SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) } == 0 {
        return Err(());
    }

    Ok(())
}

// ── Locale detection ──────────────────────────────────────────────────────────

/// Detect the user's preferred output language as a BCP-47 tag.
///
/// Priority order:
///   1. `LX_LANG` env var (explicit override, "auto" is treated as unset)
///   2. On Windows: `GetUserDefaultLocaleName` Win32 API
///   3. On Linux/macOS: `LC_ALL` → `LC_MESSAGES` → `LANG` env vars
///   4. Fallback: "en"
pub fn locale() -> String {
    // Explicit override wins across all platforms.
    if let Ok(v) = std::env::var("LX_LANG") {
        let v = v.trim().to_lowercase();
        if !v.is_empty() && v != "auto" {
            return normalize_lang_tag(&v);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(tag) = windows_locale() {
            return tag;
        }
    }

    // POSIX locale env vars (Linux/macOS and Windows fallback).
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let code = extract_lang_from_posix(&v);
            if !code.is_empty() {
                return code;
            }
        }
    }

    "en".to_string()
}

#[cfg(target_os = "windows")]
fn windows_locale() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

    // BCP-47 locale names fit comfortably in 85 UTF-16 code units (LOCALE_NAME_MAX_LENGTH).
    let mut buf = [0u16; 85];
    // SAFETY: GetUserDefaultLocaleName writes at most `buf.len()` wide chars.
    // The buffer is stack-allocated and fully owned here.
    let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return None;
    }
    // len includes the NUL terminator; exclude it.
    let tag = String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]);
    if tag.is_empty() {
        None
    } else {
        // Normalise "en-US" → "en" (lx uses 2-letter codes).
        Some(normalize_lang_tag(&tag.to_lowercase()))
    }
}

/// Extract a 2-letter language code from a POSIX locale string like "en_US.UTF-8".
fn extract_lang_from_posix(locale: &str) -> String {
    let code = locale
        .split('_')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("");
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        code.to_lowercase()
    } else {
        String::new()
    }
}

/// Normalise a BCP-47 tag to its 2-letter primary subtag.
/// "en-US", "en_US", "EN" → "en".
fn normalize_lang_tag(tag: &str) -> String {
    tag.split(['-', '_']).next().unwrap_or("en").to_lowercase()
}

// ── Shell detection ───────────────────────────────────────────────────────────

/// Detect the shell that launched the current process.
///
/// Priority order:
///   1. `LX_SHELL` env var (explicit override)
///   2. `$SHELL` env var basename (bash, zsh, fish, etc. — also set by Git Bash as `/bin/bash.exe`)
///   3. On Windows: PowerShell-exported env vars → "powershell"; otherwise "cmd"
///   4. Fallback: "bash"
///
/// Note on the Windows signal: `$PSVersionTable` is a PowerShell *automatic
/// variable*, not an environment variable, so it is never visible to a child
/// process — checking it always failed and detection fell through to "cmd"
/// even inside PowerShell. We instead key off `PSModulePath` /
/// `PSExecutionPolicyPreference`, which PowerShell *does* export to children.
/// A bare CMD session has neither, so it is correctly detected as "cmd". The
/// only ambiguous case is a CMD process spawned from within PowerShell (the
/// vars leak in) — there the user can override with `LX_SHELL` / `--shell`.
pub fn detect_shell() -> String {
    if let Ok(v) = std::env::var("LX_SHELL") {
        let v = v.trim().to_lowercase();
        if matches!(
            v.as_str(),
            "bash" | "zsh" | "sh" | "fish" | "powershell" | "cmd"
        ) {
            return v;
        }
    }

    // Strip path separators and optional .exe suffix (Git Bash sets SHELL=/bin/bash.exe).
    if let Ok(s) = std::env::var("SHELL") {
        let base = s
            .split(['/', '\\'])
            .next_back()
            .unwrap_or("")
            .to_lowercase();
        let base = base.strip_suffix(".exe").unwrap_or(&base);
        if matches!(base, "bash" | "zsh" | "sh" | "fish" | "dash" | "ksh") {
            return base.to_string();
        }
    }

    #[cfg(target_os = "windows")]
    {
        // PowerShell exports these to child processes; CMD launched directly
        // has neither. (See the doc comment above for the ambiguous case.)
        if std::env::var_os("PSModulePath").is_some()
            || std::env::var_os("PSExecutionPolicyPreference").is_some()
        {
            return "powershell".to_string();
        }
        "cmd".to_string()
    }

    #[cfg(not(target_os = "windows"))]
    "bash".to_string()
}

// ── OS identification ─────────────────────────────────────────────────────────

/// Return a stable lowercase OS identifier for use in `{os}` prompt placeholders.
///
/// Returns one of: `"linux"` | `"windows"` | `"macos"`.
/// Any other UNIX variant is mapped to `"linux"` as the closest tooling match.
pub fn os() -> &'static str {
    #[cfg(target_os = "windows")]
    return "windows";
    #[cfg(target_os = "macos")]
    return "macos";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return "linux";
}

// ── Build target ──────────────────────────────────────────────────────────────

/// The compile-time target triple embedded via build.rs.
pub fn target_triple() -> &'static str {
    option_env!("LX_TARGET").unwrap_or("unknown")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_is_absolute_or_relative_dot() {
        let dir = config_dir();
        // Must end with "lx" component.
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("lx"));
    }

    #[test]
    fn extract_lang_from_posix_handles_utf8_suffix() {
        assert_eq!(extract_lang_from_posix("de_DE.UTF-8"), "de");
        assert_eq!(extract_lang_from_posix("en_US.UTF-8"), "en");
        assert_eq!(extract_lang_from_posix("C"), "");
        assert_eq!(extract_lang_from_posix(""), "");
    }

    #[test]
    fn normalize_lang_tag_strips_region() {
        assert_eq!(normalize_lang_tag("en-US"), "en");
        assert_eq!(normalize_lang_tag("de_DE"), "de");
        assert_eq!(normalize_lang_tag("FR"), "fr");
    }

    #[test]
    fn locale_returns_nonempty_string() {
        let lang = locale();
        assert!(!lang.is_empty());
        // Must be all ASCII alphabetic (2-letter code).
        assert!(
            lang.chars().all(|c| c.is_ascii_alphabetic()),
            "unexpected: {lang}"
        );
    }

    #[test]
    fn detect_shell_returns_known_value() {
        let shell = detect_shell();
        assert!(
            matches!(
                shell.as_str(),
                "bash" | "zsh" | "sh" | "fish" | "dash" | "ksh" | "powershell" | "cmd"
            ),
            "unexpected shell: {shell}"
        );
    }

    #[test]
    fn is_tty_does_not_panic() {
        // Just verify the call doesn't panic; result depends on test runner.
        let _ = is_tty(Fd::Stdin);
        let _ = is_tty(Fd::Stdout);
        let _ = is_tty(Fd::Stderr);
    }

    #[test]
    fn os_returns_known_value() {
        assert!(matches!(os(), "linux" | "windows" | "macos"));
    }
}
