#![forbid(unsafe_code)]

use serde::Serialize;

/// Functional grouping of tools, matching §13 of the design document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    TextAnalysis,
    CodeDevelopment,
    CommandGeneration,
    FilesystemData,
    SearchKnowledge,
    ProductivityComms,
    DocsFormat,
    Security,
    NetworkSystem,
    Diagnostics,
    MetaShell,
    Web,
}

impl Category {
    pub fn display_name(self) -> &'static str {
        match self {
            Category::TextAnalysis => "Text & Analysis",
            Category::CodeDevelopment => "Code & Development",
            Category::CommandGeneration => "Command Generation",
            Category::FilesystemData => "Filesystem & Data",
            Category::SearchKnowledge => "Search & Knowledge",
            Category::ProductivityComms => "Productivity & Comms",
            Category::DocsFormat => "Docs & Format",
            Category::Security => "Security",
            Category::NetworkSystem => "Network & System",
            Category::Diagnostics => "Diagnostics",
            Category::MetaShell => "Meta & Shell",
            Category::Web => "Web",
        }
    }

    /// Short identifier used for --cat matching.
    pub fn short_id(self) -> &'static str {
        match self {
            Category::TextAnalysis => "text",
            Category::CodeDevelopment => "code",
            Category::CommandGeneration => "cmd",
            Category::FilesystemData => "fs",
            Category::SearchKnowledge => "know",
            Category::ProductivityComms => "prod",
            Category::DocsFormat => "docs",
            Category::Security => "sec",
            Category::NetworkSystem => "net",
            Category::Diagnostics => "diag",
            Category::MetaShell => "meta",
            Category::Web => "web",
        }
    }

    pub fn all() -> &'static [Category] {
        &[
            Category::TextAnalysis,
            Category::CodeDevelopment,
            Category::CommandGeneration,
            Category::FilesystemData,
            Category::SearchKnowledge,
            Category::ProductivityComms,
            Category::DocsFormat,
            Category::Security,
            Category::NetworkSystem,
            Category::Diagnostics,
            Category::MetaShell,
            Category::Web,
        ]
    }
}

/// A single tool entry in the catalog.
#[derive(Debug, Serialize)]
pub struct ToolEntry {
    /// Binary name, e.g. "lxcommit".
    pub name: &'static str,
    /// 2–3 word summary for compact multi-column display.
    pub short: &'static str,
    /// Full one-liner from §13 of the design document.
    pub purpose: &'static str,
    /// Functional category.
    pub category: Category,
}

/// All 72 tools, in §13 order.
pub const TOOLS: &[ToolEntry] = &[
    // ── 13.1 Text & Analysis ─────────────────────────────────────────────────
    ToolEntry {
        name: "lxexplain",
        short: "explain anything",
        purpose: "Explain a command, error, or code snippet in plain language",
        category: Category::TextAnalysis,
    },
    ToolEntry {
        name: "lxsum",
        short: "summarise text",
        purpose: "Summarise a file or command output (--headline for title/subject, --short for one sentence)",
        category: Category::TextAnalysis,
    },
    ToolEntry {
        name: "lxtl",
        short: "translate text",
        purpose: "Translate text to a target language (--to)",
        category: Category::TextAnalysis,
    },
    ToolEntry {
        name: "lxclass",
        short: "classify input",
        purpose: "Classify input into given labels (--labels)",
        category: Category::TextAnalysis,
    },
    ToolEntry {
        name: "lxpull",
        short: "extract fields",
        purpose: "Extract structured fields from free text (--fields)",
        category: Category::TextAnalysis,
    },
    ToolEntry {
        name: "lxproof",
        short: "fix grammar",
        purpose: "Correct grammar and spelling",
        category: Category::TextAnalysis,
    },
    // ── 13.2 Code & Development ──────────────────────────────────────────────
    ToolEntry {
        name: "lxcode",
        short: "generate code",
        purpose: "Generate code from a description (--lang)",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxdebug",
        short: "debug error",
        purpose: "Analyse an error message and suggest a fix",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxdoc",
        short: "write docstrings",
        purpose: "Generate docstrings/comments for code",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxregex",
        short: "regex pattern",
        purpose: "Generate a regex from a description (--flavor); edit existing with stdin",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxregexplain",
        short: "explain regex",
        purpose: "Explain what a regex does, in plain language, with a parts breakdown",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxsql",
        short: "generate SQL",
        purpose: "Generate SQL from natural language (--schema); edit existing with stdin",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxsh",
        short: "shell command",
        purpose: "Generate a shell command or script",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxtypehint",
        short: "add type hints",
        purpose: "Add type hints/annotations to code",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxrename",
        short: "rename script",
        purpose: "Generate a safe rename script from natural-language intent",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxfixcmd",
        short: "fix command",
        purpose: "Fix the last failed shell command",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxfixscript",
        short: "fix shell script",
        purpose: "Fix a broken shell script (--target linux|windows|macos)",
        category: Category::CodeDevelopment,
    },
    ToolEntry {
        name: "lxpatch",
        short: "generate patch",
        purpose: "Turn a described change into an applyable unified diff",
        category: Category::CodeDevelopment,
    },
    // ── 13.3 Command Generation ──────────────────────────────────────────────
    ToolEntry {
        name: "lxjq",
        short: "jq expression",
        purpose: "Generate a jq expression from a description; edit existing with stdin",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxcurl",
        short: "curl command",
        purpose: "Generate a curl command from an API description",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxsed",
        short: "sed/awk one-liner",
        purpose: "Generate a sed or awk text-transformation one-liner",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxffmpeg",
        short: "ffmpeg command",
        purpose: "Generate an ffmpeg command",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxkubectl",
        short: "kubectl command",
        purpose: "Generate a kubectl command",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxdockercmd",
        short: "docker command",
        purpose: "Generate a docker command",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxrsync",
        short: "rsync command",
        purpose: "Generate an rsync command (data-loss aware)",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxmount",
        short: "mount command",
        purpose: "Generate a mount command and fstab line (--target linux|windows|macos); stateful",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxkill",
        short: "kill process",
        purpose: "Find and kill the right process from a description (--target linux|windows|macos)",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxcron",
        short: "crontab line",
        purpose: "Generate or explain a crontab line; edit existing with stdin",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxfirewall",
        short: "firewall rule",
        purpose: "Generate or explain firewall rules (--target linux|windows|macos); stateful",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxip",
        short: "ip command",
        purpose: "Generate or explain ip addr/route/link commands (--target linux|windows|macos); stateful",
        category: Category::CommandGeneration,
    },
    ToolEntry {
        name: "lxprintf",
        short: "printf format",
        purpose: "Build a printf/date format string from a description",
        category: Category::CommandGeneration,
    },
    // ── 13.4 Filesystem & Data ───────────────────────────────────────────────
    ToolEntry {
        name: "lxfind",
        short: "semantic file search",
        purpose: "Semantic file search by description",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxgrep",
        short: "semantic grep",
        purpose: "Semantic content search",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxdigest",
        short: "summarise directory",
        purpose: "Summarise a whole directory",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxcsv",
        short: "query CSV",
        purpose: "Answer questions about CSV data",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxjson",
        short: "repair JSON",
        purpose: "Repair or clean malformed JSON",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxconv",
        short: "convert format",
        purpose: "Convert between data formats (--to)",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxtable",
        short: "text to table",
        purpose: "Convert unstructured text into a table",
        category: Category::FilesystemData,
    },
    ToolEntry {
        name: "lxmock",
        short: "generate mock data",
        purpose: "Generate realistic mock/fixture data from a description",
        category: Category::FilesystemData,
    },
    // ── 13.5 Search & Knowledge ──────────────────────────────────────────────
    ToolEntry {
        name: "lxask",
        short: "answer question",
        purpose: "Answer a question from local context (--context) or knowledge",
        category: Category::SearchKnowledge,
    },
    ToolEntry {
        name: "lxman",
        short: "plain-language man",
        purpose: "Plain-language man page for a command",
        category: Category::SearchKnowledge,
    },
    ToolEntry {
        name: "lxerrno",
        short: "explain error code",
        purpose: "Explain an error code (HTTP/errno/exit)",
        category: Category::SearchKnowledge,
    },
    // ── 13.6 Productivity & Comms ────────────────────────────────────────────
    ToolEntry {
        name: "lxdraft",
        short: "draft email/doc",
        purpose: "Draft an email/ticket/doc from bullet points (--kind)",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxcommit",
        short: "commit message",
        purpose: "Generate a Conventional Commit message from a git diff",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxclog",
        short: "changelog",
        purpose: "Generate a changelog from git log",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxpr",
        short: "PR description",
        purpose: "Generate a PR description from a diff",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxstandup",
        short: "standup notes",
        purpose: "Generate a standup from git activity",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxtodo",
        short: "extract TODOs",
        purpose: "Extract TODO comments from code",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxnotes",
        short: "structure notes",
        purpose: "Structure raw meeting notes (--actions to extract action items)",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxgitignore",
        short: "generate .gitignore",
        purpose: "Generate a .gitignore for a project; edit existing with stdin",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxdockerfile",
        short: "generate Dockerfile",
        purpose: "Generate a Dockerfile; edit existing with stdin",
        category: Category::ProductivityComms,
    },
    ToolEntry {
        name: "lxmakefile",
        short: "generate Makefile",
        purpose: "Generate a Makefile/justfile; edit existing with stdin",
        category: Category::ProductivityComms,
    },
    // ── 13.7 Docs & Format ───────────────────────────────────────────────────
    ToolEntry {
        name: "lxmd",
        short: "format as Markdown",
        purpose: "Format raw text as clean Markdown",
        category: Category::DocsFormat,
    },
    ToolEntry {
        name: "lxmermaid",
        short: "Mermaid diagram",
        purpose: "Generate a Mermaid diagram; edit existing with stdin",
        category: Category::DocsFormat,
    },
    ToolEntry {
        name: "lxdiff",
        short: "explain diff",
        purpose: "Explain a git/file diff in plain language",
        category: Category::DocsFormat,
    },
    ToolEntry {
        name: "lxgraph",
        short: "ASCII chart",
        purpose: "Generate an ASCII/terminal chart from numbers",
        category: Category::DocsFormat,
    },
    // ── 13.8 Security ────────────────────────────────────────────────────────
    ToolEntry {
        name: "lxsecret",
        short: "scan for secrets",
        purpose: "Find accidentally committed secrets/keys",
        category: Category::Security,
    },
    ToolEntry {
        name: "lxredact",
        short: "redact secrets",
        purpose: "Mask secrets and PII in a data stream (--anon to replace names with roles)",
        category: Category::Security,
    },
    ToolEntry {
        name: "lxperm",
        short: "explain permissions",
        purpose: "Explain file permissions and risks",
        category: Category::Security,
    },
    ToolEntry {
        name: "lxcve",
        short: "explain CVE",
        purpose: "Explain CVEs in a dependency lockfile",
        category: Category::Security,
    },
    ToolEntry {
        name: "lxcert",
        short: "explain TLS cert",
        purpose: "Explain a TLS certificate",
        category: Category::Security,
    },
    ToolEntry {
        name: "lxjwt",
        short: "decode JWT",
        purpose: "Decode and explain a JWT token",
        category: Category::Security,
    },
    ToolEntry {
        name: "lxchmod",
        short: "safe permissions",
        purpose: "Suggest safe file permissions",
        category: Category::Security,
    },
    // ── 13.9 Network & System ────────────────────────────────────────────────
    ToolEntry {
        name: "lxlog",
        short: "analyse logs",
        purpose: "Analyse logs and detect anomalies",
        category: Category::NetworkSystem,
    },
    ToolEntry {
        name: "lxconf",
        short: "check config file",
        purpose: "Check a config file for typical errors; edit existing with stdin",
        category: Category::NetworkSystem,
    },
    ToolEntry {
        name: "lxport",
        short: "explain port",
        purpose: "Explain what service runs on a port and flag any risk",
        category: Category::NetworkSystem,
    },
    // ── 13.10 Diagnostics ────────────────────────────────────────────────────
    ToolEntry {
        name: "lxdns",
        short: "diagnose DNS",
        purpose: "Diagnose DNS problems from dig/nslookup/host output",
        category: Category::Diagnostics,
    },
    ToolEntry {
        name: "lxssl",
        short: "diagnose TLS",
        purpose: "Diagnose TLS/cert errors from openssl/curl output",
        category: Category::Diagnostics,
    },
    ToolEntry {
        name: "lxping",
        short: "interpret ping",
        purpose: "Interpret ping/traceroute/mtr: network vs host problem",
        category: Category::Diagnostics,
    },
    ToolEntry {
        name: "lxhttp",
        short: "explain HTTP failure",
        purpose: "Explain why an HTTP request failed (paste curl -v output)",
        category: Category::Diagnostics,
    },
    // ── 13.11 Meta & Shell ───────────────────────────────────────────────────
    ToolEntry {
        name: "lxundo",
        short: "undo command",
        purpose: "Suggest how to undo a command",
        category: Category::MetaShell,
    },
    // ── 13.12 Web ────────────────────────────────────────────────────────────
    ToolEntry {
        name: "lxurl",
        short: "fetch and answer",
        purpose: "Fetch a URL and answer questions about its content",
        category: Category::Web,
    },
];
