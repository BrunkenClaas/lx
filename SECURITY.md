# Security Policy

LX Coreutils is a suite of security-conscious tools: several redact secrets and
PII before any data leaves the machine, and several generate shell commands,
SQL, or scripts. That makes a few classes of bug security-relevant. This
document says which ones, and how to report them.

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Report it privately through GitHub's
[private vulnerability advisories](https://github.com/BrunkenClaas/lx/security/advisories/new)
(the "Report a vulnerability" button on the repository's **Security** tab). This
keeps the report confidential until a fix is available and lets the maintainer
respond and credit you through GitHub without exposing any contact details.

Please include:

- The tool and version (`lx<tool> --version`).
- A minimal reproduction — input, command, and observed output. **Use synthetic
  secrets**, never a real credential.
- The provider/model in use, if relevant.

Expect an acknowledgement within a few days. Because this is a solo-maintained
project, fixes are best-effort rather than bound to a fixed SLA; a private
advisory and credit follow once a fix is available.

## What counts as a vulnerability here

- **A real secret or PII reaching the LLM request** from a tool that is supposed
  to redact it (the `redact` security flag). Redaction is best-effort by design —
  see the note below — but a *systematic* miss of a documented secret format is a
  bug worth reporting.
- **A tool executing, or persisting, a command it generated.** The suite's core
  invariant is that generators write text to stdout and nothing more — they never
  run commands, never touch shell profiles, crontab, the registry, or autostart.
  Any deviation is a serious bug.
- **A `fsbound` tool escaping its allowed root** (path traversal, symlink escape)
  to read files outside the path the user specified.
- **A `nocmd` tool emitting a dangerous command with no danger warning**, or
  exiting 0 when it should have flagged and refused.

## What is *not* a vulnerability

- **Weak model output.** A small local model producing a wrong or low-quality
  answer is a model-quality limitation, not a security hole. Try a larger model.
- **Redaction missing a novel or obfuscated secret format.** Redaction is a
  best-effort defence-in-depth layer, not a guarantee — this is documented in the
  README and design doc. Report a *systematic* miss of a *documented* format; a
  one-off exotic string is an enhancement request instead.
- **Prompt-injection content in untrusted input changing a summary's wording.**
  `untrusted` tools are instructed to ignore embedded instructions, but treat any
  LLM output as untrusted data regardless. A wording change is not a breach; a
  tool being coerced into *executing* something would be.

## Scope

This policy covers the tools and library crates in this repository. It does not
cover the upstream LLM providers, Ollama, or your own model deployment.
