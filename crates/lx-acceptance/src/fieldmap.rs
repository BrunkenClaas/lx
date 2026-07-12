//! Tool -> JSON output field maps.
//!
//! Every productive tool emits its full `Output` struct on stdout under
//! `--json`. The *main field* is the one that goes to stdout in plain mode (the
//! result a user would pipe). The *danger field* is the boolean set locally by
//! `nocmd` tools to signal a dangerous command.
//!
//! These maps were derived directly from each tool's `Output` struct in
//! `tools/<tool>/src/run.rs` (the first field of the primary `Output`/`*Output`
//! struct). They are the single source of truth; an intent may override the main
//! field per-entry via `json_field`.

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// The field that carries the primary result for each tool.
static MAIN_FIELD: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("lxask", "answer"),
        ("lxcert", "subject"),
        ("lxchmod", "suggestion"),
        ("lxclass", "label"),
        ("lxclog", "entries"),
        ("lxcode", "code"),
        // CommitOutput serializes commit_type as "type" (serde rename).
        ("lxcommit", "type"),
        ("lxconf", "findings"),
        ("lxconv", "content"),
        ("lxcron", "crontab"),
        ("lxcsv", "answer"),
        ("lxcurl", "command"),
        ("lxcve", "vulns"),
        ("lxdebug", "cause"),
        ("lxdiff", "summary"),
        ("lxdigest", "summary"),
        ("lxdns", "explanation"),
        ("lxdoc", "code"),
        ("lxdockercmd", "command"),
        ("lxdockerfile", "content"),
        ("lxdraft", "subject"),
        ("lxerrno", "code"),
        ("lxexplain", "summary"),
        ("lxffmpeg", "command"),
        ("lxfind", "paths"),
        ("lxfirewall", "command"),
        ("lxfixcmd", "command"),
        ("lxfixscript", "script"),
        ("lxgitignore", "content"),
        ("lxgraph", "chart"),
        ("lxgrep", "matches"),
        ("lxhttp", "explanation"),
        ("lxip", "command"),
        ("lxjq", "expression"),
        ("lxjson", "json"),
        ("lxjwt", "header"),
        ("lxkill", "command"),
        ("lxkubectl", "command"),
        ("lxlog", "anomalies"),
        ("lxmakefile", "content"),
        ("lxman", "summary"),
        ("lxmd", "markdown"),
        ("lxmermaid", "diagram"),
        ("lxmock", "data"),
        ("lxmount", "command"),
        ("lxnotes", "sections"),
        ("lxpatch", "diff"),
        ("lxperm", "items"),
        ("lxping", "explanation"),
        ("lxport", "port"),
        ("lxpr", "title"),
        ("lxprintf", "format"),
        ("lxproof", "text"),
        ("lxpull", "records"),
        ("lxredact", "redacted_text"),
        ("lxregex", "pattern"),
        ("lxregexplain", "regex"),
        ("lxrename", "renames"),
        ("lxrsync", "command"),
        ("lxsecret", "findings"),
        ("lxsed", "command"),
        ("lxsh", "command"),
        ("lxsql", "sql"),
        ("lxssl", "explanation"),
        ("lxstandup", "done"),
        ("lxsum", "tldr"),
        ("lxtable", "columns"),
        ("lxtl", "text"),
        ("lxtodo", "todos"),
        ("lxtypehint", "code"),
        ("lxundo", "undo_command"),
        ("lxurl", "url"),
    ]
    .into_iter()
    .collect()
});

/// Returns the default main-result field for a tool, if known.
pub fn main_field(tool: &str) -> Option<&'static str> {
    MAIN_FIELD.get(tool).copied()
}

/// Returns the danger boolean field name for a tool. Almost all `nocmd` tools
/// use `dangerous`; lxsql is the lone exception (`mutating`). Verified against
/// `tools/<tool>/src/run.rs`: only lxsql declares `pub mutating: bool`.
pub fn danger_field(tool: &str) -> &'static str {
    match tool {
        "lxsql" => "mutating",
        _ => "dangerous",
    }
}
