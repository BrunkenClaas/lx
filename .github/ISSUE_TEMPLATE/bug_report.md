---
name: Bug report
about: A tool produced wrong output, crashed, or behaved against its documented contract
title: "[bug] lx<tool>: "
labels: bug
---

<!--
Before filing: which tool? Run `lx<tool> --version` and paste the line below.
A bug here means a tool broke its documented contract (docs/design_document.md
§13). "The model gave a weak answer on a 3B model" is usually not a bug — that's
model quality; try a larger local model or a hosted provider first.
-->

**Tool and version**

```
$ lx<tool> --version
```

**What I ran**

```sh
# exact command, with input (redact any real secrets — the suite masks them,
# but a bug report is public)
```

**What I expected**

**What happened instead**

```
# actual stdout / stderr, exit code
```

**Provider / model**

- Provider: <!-- ollama / anthropic / openai / … -->
- Model: <!-- e.g. qwen2.5:7b -->

**Environment**

- OS: <!-- Linux / macOS / Windows + version -->
- Install: <!-- release ZIP / built from source -->
