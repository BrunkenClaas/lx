# lxkubectl

Generate a kubectl command from a plain-English description.

## Usage

```
lxkubectl [OPTIONS] [DESCRIPTION]
```

`DESCRIPTION` is the natural-language description of the kubectl command to generate.
If omitted, reads from stdin or `--file`.

## Examples

```sh
lxkubectl "list all pods in the production namespace"
# kubectl get pods -n production

lxkubectl "delete all pods with label app=nginx in staging"
# DANGER: command contains 'kubectl delete' ...
# kubectl delete pods -l app=nginx -n staging

lxkubectl --json "scale the api deployment to 5 replicas"
# {"command":"kubectl scale deployment api --replicas=5","dangerous":false}

lxkubectl --dry-run "drain node worker-1"
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON (`command`, `dangerous`) to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show the description that would be sent to the LLM, then exit |
| `--quiet` / `-q` | Suppress informational stderr (DANGER warnings are never suppressed) |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--verbose` | Show verbose diagnostics on stderr |
| `--max-input-bytes <n>` | Limit input size (default: 512 KiB) |
| `--file <PATH>` | Read description from file |
| `--version` / `-V` | Print version information |
| `--help` / `-h` | Print help |

## Security flags

`nocmd` — This tool never executes any command. It only prints the generated kubectl
command to stdout. Before outputting, it checks locally for dangerous patterns:

- `kubectl delete` — permanently removes Kubernetes resources
- `kubectl drain` — evicts all pods from a node
- `kubectl cordon` — marks a node unschedulable
- `kubectl exec` — executes commands inside a running container
- `--all-namespaces` with destructive operations

Dangerous commands are flagged with a `DANGER:` message on stderr. This warning
is never suppressed, even with `--quiet`.

## Output

**Plain mode** (default): the kubectl command only on stdout, explanation on stderr.

**JSON mode** (`--json`):
```json
{
  "command": "kubectl get pods -n production -o wide",
  "dangerous": false
}
```
