# lxcve

Explain CVEs in a dependency lockfile

## Output

Each finding includes `package`, `version`, `cve_id`, `severity` (CRITICAL/HIGH/MEDIUM/LOW),
`description`, and `confidence` (high/medium/low). Confidence reflects the model's certainty
about the CVE ID and version range — always verify findings against NVD or OSV before acting
on them.

## Security flags

nonet fsbound untrusted
