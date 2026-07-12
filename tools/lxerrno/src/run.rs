#![forbid(unsafe_code)]
// The lib target does not call run() directly; dead_code warnings are expected
// for internal helpers that are exercised only via the binary or integration tests.
#![allow(dead_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Output produced by `lxerrno`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The normalised code string (e.g. "404", "ENOENT", "130")
    pub code: String,
    /// One-sentence human-readable meaning
    pub meaning: String,
    /// Optional actionable hint (empty string when absent)
    #[serde(default)]
    pub hint: String,
}

impl Output {
    /// Render as human-readable plain text.
    pub fn to_plain(&self) -> String {
        let mut out = format!("{}: {}\n", self.code, self.meaning);
        if !self.hint.is_empty() {
            out.push_str(&format!("  hint: {}\n", self.hint));
        }
        out
    }
}

// ── Local lookup tables ────────────────────────────────────────────────────────

struct StaticCode {
    code: &'static str,
    meaning: &'static str,
    hint: &'static str,
}

/// Well-known HTTP status codes.
const HTTP_CODES: &[StaticCode] = &[
    StaticCode {
        code: "100",
        meaning:
            "Continue — the server received the request headers and the client should proceed.",
        hint: "Used in Expect: 100-continue flows; usually handled automatically by HTTP clients.",
    },
    StaticCode {
        code: "101",
        meaning: "Switching Protocols — the server agrees to switch protocols as requested.",
        hint: "Common for WebSocket upgrades.",
    },
    StaticCode {
        code: "200",
        meaning: "OK — the request succeeded.",
        hint: "Standard success response for GET, POST, PUT, PATCH, DELETE.",
    },
    StaticCode {
        code: "201",
        meaning: "Created — the request succeeded and a new resource was created.",
        hint: "Typical for POST requests that create a resource.",
    },
    StaticCode {
        code: "204",
        meaning: "No Content — the request succeeded but there is no response body.",
        hint: "Common for DELETE requests or PATCH with no return value.",
    },
    StaticCode {
        code: "301",
        meaning: "Moved Permanently — the resource has been permanently moved to a new URL.",
        hint: "Update bookmarks and links to the new URL; clients should cache this redirect.",
    },
    StaticCode {
        code: "302",
        meaning: "Found — the resource is temporarily at a different URL.",
        hint: "Do not cache this redirect; the original URL may be used again.",
    },
    StaticCode {
        code: "304",
        meaning: "Not Modified — the cached version is still valid.",
        hint: "The client should use its cached copy; no body is returned.",
    },
    StaticCode {
        code: "307",
        meaning: "Temporary Redirect — same as 302 but the method must not change.",
        hint: "Use when the redirect must preserve POST or PUT semantics.",
    },
    StaticCode {
        code: "308",
        meaning: "Permanent Redirect — same as 301 but the method must not change.",
        hint: "Use when the permanent redirect must preserve POST or PUT semantics.",
    },
    StaticCode {
        code: "400",
        meaning: "Bad Request — the server could not understand the request due to invalid syntax.",
        hint: "Check the request body, headers, and query parameters for malformed data.",
    },
    StaticCode {
        code: "401",
        meaning: "Unauthorized — authentication is required and has failed or not been provided.",
        hint: "Provide valid credentials via Authorization header or re-authenticate.",
    },
    StaticCode {
        code: "403",
        meaning: "Forbidden — the server understood the request but refuses to authorise it.",
        hint: "Check permissions; valid credentials do not grant access to this resource.",
    },
    StaticCode {
        code: "404",
        meaning: "Not Found — the requested resource could not be found on the server.",
        hint: "Verify the URL path, check for typos, and confirm the resource exists.",
    },
    StaticCode {
        code: "405",
        meaning: "Method Not Allowed — the HTTP method is not allowed for this endpoint.",
        hint: "Check the Allow response header for the list of permitted methods.",
    },
    StaticCode {
        code: "408",
        meaning: "Request Timeout — the server timed out waiting for the request.",
        hint: "Retry the request; consider increasing timeout settings on the client.",
    },
    StaticCode {
        code: "409",
        meaning: "Conflict — the request conflicts with the current state of the resource.",
        hint: "Often seen with concurrent writes; resolve the conflict and retry.",
    },
    StaticCode {
        code: "410",
        meaning:
            "Gone — the resource has been permanently deleted and will not be available again.",
        hint: "Unlike 404, this is definitive; remove any links or references to this URL.",
    },
    StaticCode {
        code: "422",
        meaning: "Unprocessable Entity — the request was well-formed but contains semantic errors.",
        hint: "Check the response body for field-level validation errors.",
    },
    StaticCode {
        code: "429",
        meaning:
            "Too Many Requests — the client has sent too many requests in a given time window.",
        hint: "Back off and retry after the delay specified in the Retry-After header.",
    },
    StaticCode {
        code: "500",
        meaning: "Internal Server Error — the server encountered an unexpected condition.",
        hint: "This is a server-side bug; retry later or contact the service owner.",
    },
    StaticCode {
        code: "502",
        meaning: "Bad Gateway — the server received an invalid response from an upstream server.",
        hint: "Often a temporary proxy or load-balancer issue; retry after a short wait.",
    },
    StaticCode {
        code: "503",
        meaning: "Service Unavailable — the server is temporarily unable to handle the request.",
        hint: "Check the Retry-After header; the service may be overloaded or under maintenance.",
    },
    StaticCode {
        code: "504",
        meaning: "Gateway Timeout — the upstream server did not respond in time.",
        hint: "Retry later; if persistent, investigate network latency between servers.",
    },
];

/// Well-known POSIX errno codes (number + name).
const ERRNO_CODES: &[StaticCode] = &[
    StaticCode { code: "EPERM/1",         meaning: "Operation not permitted — the caller lacks the required privilege.", hint: "Run as root or check file/process ownership." },
    StaticCode { code: "ENOENT/2",        meaning: "No such file or directory — the specified path does not exist.", hint: "Verify the path for typos and confirm the file or directory was created." },
    StaticCode { code: "ESRCH/3",         meaning: "No such process — the target process ID does not exist.", hint: "The process may have already exited; check with `ps`." },
    StaticCode { code: "EINTR/4",         meaning: "Interrupted system call — a signal interrupted the operation before completion.", hint: "Retry the operation; consider using SA_RESTART for signal handlers." },
    StaticCode { code: "EIO/5",           meaning: "Input/output error — a hardware or driver-level I/O failure occurred.", hint: "Check disk health with `smartctl`; the device may be failing." },
    StaticCode { code: "ENOEXEC/8",       meaning: "Exec format error — the file is not a valid executable for this architecture.", hint: "Confirm the binary matches the host architecture (e.g., arm64 vs x86_64)." },
    StaticCode { code: "EBADF/9",         meaning: "Bad file descriptor — the file descriptor is not open or is invalid.", hint: "Ensure the file was opened successfully before using the descriptor." },
    StaticCode { code: "ECHILD/10",       meaning: "No child processes — wait() was called with no children to wait for.", hint: "Only call waitpid/wait after forking a child process." },
    StaticCode { code: "EAGAIN/11",       meaning: "Resource temporarily unavailable — try again (also EWOULDBLOCK).", hint: "For non-blocking I/O, retry after a short delay or use select/poll/epoll." },
    StaticCode { code: "ENOMEM/12",       meaning: "Out of memory — the kernel could not allocate the requested memory.", hint: "Free memory or increase available RAM/swap; check for memory leaks." },
    StaticCode { code: "EACCES/13",       meaning: "Permission denied — the file system permission check failed.", hint: "Check file mode bits and ownership with `ls -l`; adjust with `chmod`/`chown`." },
    StaticCode { code: "EFAULT/14",       meaning: "Bad address — a pointer argument points outside the accessible address space.", hint: "This usually indicates a bug in the calling code; check pointer validity." },
    StaticCode { code: "EBUSY/16",        meaning: "Device or resource busy — the resource is locked or in use.", hint: "Unmount file systems or stop processes using the resource before retrying." },
    StaticCode { code: "EEXIST/17",       meaning: "File exists — cannot create a file that already exists.", hint: "Remove the existing file first, or use O_TRUNC/O_RDWR to open it instead." },
    StaticCode { code: "ENODEV/19",       meaning: "No such device — the device does not exist or is not configured.", hint: "Check that the device is connected and the kernel module is loaded." },
    StaticCode { code: "ENOTDIR/20",      meaning: "Not a directory — a path component that was expected to be a directory is not.", hint: "Verify that every component of the path is a directory, not a file." },
    StaticCode { code: "EISDIR/21",       meaning: "Is a directory — the operation requires a regular file but a directory was given.", hint: "Use the correct path to a regular file, not a directory." },
    StaticCode { code: "EINVAL/22",       meaning: "Invalid argument — an argument to a system call had an invalid value.", hint: "Check all arguments; consult the man page for valid ranges and types." },
    StaticCode { code: "ENFILE/23",       meaning: "Too many open files in system — the system-wide open file limit is reached.", hint: "Increase `fs.file-max` via sysctl or close files in other processes." },
    StaticCode { code: "EMFILE/24",       meaning: "Too many open files — the per-process open file descriptor limit is reached.", hint: "Increase `ulimit -n` or close unused file descriptors in the application." },
    StaticCode { code: "ENOSPC/28",       meaning: "No space left on device — the file system is full.", hint: "Free disk space with `df -h` and `du -sh *`; remove unneeded files." },
    StaticCode { code: "EPIPE/32",        meaning: "Broken pipe — a write to a pipe or socket with no readers failed.", hint: "The reader closed the connection; handle SIGPIPE or check the reader." },
    StaticCode { code: "ERANGE/34",       meaning: "Numerical result out of range — a function returned a value outside representable bounds.", hint: "Check for overflow or use a wider numeric type." },
    StaticCode { code: "EDEADLK/35",      meaning: "Resource deadlock avoided — granting the lock would cause a deadlock.", hint: "Review lock acquisition order; release held locks before retrying." },
    StaticCode { code: "ENAMETOOLONG/36", meaning: "File name too long — the path exceeds the filesystem's maximum length (usually 255 bytes).", hint: "Shorten the file or directory name." },
    StaticCode { code: "ENOSYS/38",       meaning: "Function not implemented — the system call is not supported on this kernel or platform.", hint: "Check kernel version or use an alternative API." },
    StaticCode { code: "ENOTEMPTY/39",    meaning: "Directory not empty — rmdir() requires an empty directory.", hint: "Remove all contents first, or use `rm -rf` for recursive deletion." },
    StaticCode { code: "EOVERFLOW/75",    meaning: "Value too large for defined data type — a value exceeds the type's maximum.", hint: "Use a larger integer type (e.g., off64_t) or enable large-file support." },
    StaticCode { code: "ETIMEDOUT/110",   meaning: "Connection timed out — the operation exceeded its time limit.", hint: "Retry; check network connectivity and increase timeout if appropriate." },
    StaticCode { code: "ECONNREFUSED/111",meaning: "Connection refused — no process is listening on the remote address and port.", hint: "Verify the server is running and listening on the expected port." },
    StaticCode { code: "EHOSTUNREACH/113",meaning: "No route to host — the network is unreachable or the host is down.", hint: "Check routing tables with `ip route` and confirm the host is reachable." },
];

/// Well-known shell/process exit codes.
const EXIT_CODES: &[StaticCode] = &[
    StaticCode {
        code: "exit 0",
        meaning: "Success — the command completed without errors.",
        hint: "",
    },
    StaticCode {
        code: "exit 1",
        meaning: "General error — the command failed for an unspecified reason.",
        hint: "Check the command's stderr output for more details.",
    },
    StaticCode {
        code: "exit 2",
        meaning: "Misuse of shell built-in — incorrect usage or invalid arguments.",
        hint: "Run the command with --help to review correct usage.",
    },
    StaticCode {
        code: "exit 126",
        meaning: "Command cannot execute — the file exists but is not executable.",
        hint: "Add execute permission with `chmod +x <file>`.",
    },
    StaticCode {
        code: "exit 127",
        meaning: "Command not found — the shell could not locate the executable.",
        hint: "Check that the command is installed and on PATH.",
    },
    StaticCode {
        code: "exit 128",
        meaning:
            "Invalid exit argument — exit was called with a non-integer or out-of-range value.",
        hint: "Exit codes must be integers in the range 0–255.",
    },
    StaticCode {
        code: "exit 130",
        meaning: "Terminated by SIGINT (signal 2) — the process was interrupted by Ctrl-C.",
        hint: "",
    },
    StaticCode {
        code: "exit 137",
        meaning: "Killed by SIGKILL (signal 9) — the process was forcefully terminated.",
        hint: "Often caused by the OOM killer; check available memory.",
    },
    StaticCode {
        code: "exit 143",
        meaning:
            "Terminated by SIGTERM (signal 15) — the process received a graceful shutdown signal.",
        hint: "Sent by `kill` or process managers like systemd; handle it for clean shutdown.",
    },
];

// ── Parser ─────────────────────────────────────────────────────────────────────

/// Classify and normalise the raw input token.
enum CodeKind {
    Http(u16),
    Errno {
        name: Option<String>,
        num: Option<u32>,
    },
    Exit(u32),
    /// Opaque string — pass directly to the LLM (value unused; `run()` uses the original `trimmed`).
    Unknown,
}

fn parse_input(input: &str) -> CodeKind {
    let trimmed = input.trim();

    // "exit 130", "exit130", "exitcode 0"
    if let Some(rest) = trimmed
        .strip_prefix("exit ")
        .or_else(|| trimmed.strip_prefix("exitcode "))
        .or_else(|| trimmed.strip_prefix("exit"))
    {
        if let Ok(n) = rest.trim().parse::<u32>() {
            return CodeKind::Exit(n);
        }
    }

    // POSIX name like "ENOENT", "enoent"
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with('E') && upper.chars().all(|c| c.is_ascii_alphabetic()) && upper.len() >= 3
    {
        return CodeKind::Errno {
            name: Some(upper),
            num: None,
        };
    }

    // Pure numeric
    if let Ok(n) = trimmed.parse::<u32>() {
        // Heuristic: 100–599 → HTTP, 1–200 → errno (also covers some HTTP), >255 might be exit
        // We'll try HTTP first for values in the 3-digit HTTP range, errno otherwise.
        if (100..=599).contains(&n) {
            return CodeKind::Http(n as u16);
        }
        if n <= 200 {
            return CodeKind::Errno {
                name: None,
                num: Some(n),
            };
        }
        // 201–255 might be errno overflow or exit codes
        return CodeKind::Errno {
            name: None,
            num: Some(n),
        };
    }

    // "errno 28", "errno: ENOSPC"
    if let Some(rest) = upper
        .strip_prefix("ERRNO ")
        .or_else(|| upper.strip_prefix("ERRNO:"))
    {
        let rest = rest.trim().trim_start_matches(':').trim();
        if let Ok(n) = rest.parse::<u32>() {
            return CodeKind::Errno {
                name: None,
                num: Some(n),
            };
        }
        if rest.starts_with('E') && rest.chars().all(|c| c.is_ascii_alphabetic()) {
            return CodeKind::Errno {
                name: Some(rest.to_string()),
                num: None,
            };
        }
    }

    // "http 404", "http: 502"
    if let Some(rest) = upper
        .strip_prefix("HTTP ")
        .or_else(|| upper.strip_prefix("HTTP:"))
    {
        let rest = rest.trim().trim_start_matches(':').trim();
        if let Ok(n) = rest.parse::<u16>() {
            return CodeKind::Http(n);
        }
    }

    CodeKind::Unknown
}

fn lookup_local(kind: &CodeKind) -> Option<Output> {
    match kind {
        CodeKind::Http(n) => {
            let key = n.to_string();
            HTTP_CODES.iter().find(|e| e.code == key).map(|e| Output {
                code: format!("HTTP {}", e.code),
                meaning: e.meaning.to_string(),
                hint: e.hint.to_string(),
            })
        }
        CodeKind::Errno { name, num } => {
            // Try by name first, then by number in the slash-separated "NAME/NUM" key.
            ERRNO_CODES
                .iter()
                .find(|e| {
                    if let Some(ref n) = name {
                        e.code
                            .split('/')
                            .next()
                            .map(|s| s == n.as_str())
                            .unwrap_or(false)
                    } else if let Some(n) = num {
                        e.code
                            .split('/')
                            .nth(1)
                            .and_then(|s| s.parse::<u32>().ok())
                            .map(|v| v == *n)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                })
                .map(|e| Output {
                    code: e.code.to_string(),
                    meaning: e.meaning.to_string(),
                    hint: e.hint.to_string(),
                })
        }
        CodeKind::Exit(n) => {
            // Also handle 128+N (signal) generically.
            let key = format!("exit {}", n);
            if let Some(entry) = EXIT_CODES.iter().find(|e| e.code == key) {
                return Some(Output {
                    code: entry.code.to_string(),
                    meaning: entry.meaning.to_string(),
                    hint: entry.hint.to_string(),
                });
            }
            // 128 + N where N > 0 → killed by signal N
            if *n > 128 && *n <= 192 {
                let sig = n - 128;
                return Some(Output {
                    code:    format!("exit {n}"),
                    meaning: format!("Killed by signal {sig} (128+{sig}) — the process was terminated by an OS signal.", sig = sig),
                    hint:    format!("Signal {sig}: use `kill -l {sig}` to identify the signal name.", sig = sig),
                });
            }
            None
        }
        CodeKind::Unknown => None,
    }
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Core logic for `lxerrno`.
///
/// Pure function: no I/O, no `process::exit`. Testable with `MockLlmClient`.
///
/// Well-known codes (HTTP, POSIX errno, exit codes) are resolved locally
/// without an LLM call. Unknown codes fall back to the LLM.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(LxError::BadUsage("no error code provided".to_string()));
    }

    let kind = parse_input(trimmed);

    // Fast path: local lookup, no network.
    if let Some(out) = lookup_local(&kind) {
        return Ok(out);
    }

    // Slow path: send to LLM.
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
    let req = Request {
        system: &system,
        user: trimmed,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };
    let resp = client.complete(&req).map_err(LxError::from)?;
    parse_response::<Output>(&resp.content)
}
