use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, inject_os, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Output of `lxkill`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub command: String,
    pub target: String,
    pub reason: String,
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.command.clone()
    }
}

/// Check whether a kill command targets a critical system process.
///
/// This check is deterministic and local — never delegated to the LLM.
/// Returns `true` if any dangerous pattern was found, and prints a warning to stderr.
/// Return `true` if the command may kill a critical system process. Pure — no I/O.
///
/// main.rs emits the tier-3 danger warning via [`warn_danger`]; this only decides.
pub fn check(cmd: &str) -> bool {
    let c = cmd.to_lowercase();

    // Direct PID-1 kills (with or without trailing newline/space)
    let hits_pid1 = c.trim_end() == "kill 1"
        || c.trim_end() == "kill -9 1"
        || c.trim_end() == "kill -15 1"
        || c.contains("kill 1 ")
        || c.contains("kill -9 1 ")
        || c.contains("kill -15 1 ")
        || c.contains("kill 1\n")
        || c.contains("kill -9 1\n");

    // Broadcast kills (kill -9 -1 sends SIGKILL to all processes)
    let hits_broadcast = c.contains("kill -9 -1") || c.contains("pkill -9 -1");

    // Critical process names
    let hits_critical = c.contains("killall init")
        || c.contains("killall systemd")
        || c.contains("pkill -9 init")
        || c.contains("pkill systemd")
        || c.contains("pkill init");

    hits_pid1 || hits_broadcast || hits_critical
}

/// Emit the tier-3 danger warning on stderr (always shown, never suppressed by --quiet).
/// No-op when `dangerous` is false.
pub fn warn_danger(dangerous: bool) {
    if dangerous {
        eprintln!("DANGER: This command may kill a critical system process.");
        eprintln!("   Review carefully before running. This command was NOT executed.");
    }
}

/// Core logic for lxkill.
///
/// Generates a shell command to find and kill the described process.
/// `target_os` is one of `"linux" | "windows" | "macos"`.
/// NEVER executes any command. Checks for dangerous patterns locally.
pub fn run(
    description: &str,
    context: &str,
    target_os: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, bool), LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no description provided; pass the process description as an argument".to_string(),
        ));
    }

    let system = inject_os(
        &inject_lang(SYSTEM_TEMPLATE, &config.output.lang),
        target_os,
    );

    let user_msg = if context.trim().is_empty() {
        description.trim().to_string()
    } else {
        format!(
            "Description: {}\n\nProcess list context:\n{}",
            description.trim(),
            context.trim()
        )
    };

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.command.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty command".to_string(),
        ));
    }

    // Local danger detection — deterministic, not delegated to the LLM.
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let locally_dangerous = check(&out.command);
    if locally_dangerous {
        out.dangerous = true;
    }

    Ok((out, locally_dangerous))
}
