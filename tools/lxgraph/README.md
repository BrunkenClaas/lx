# lxgraph

Generate an ASCII bar chart from numeric data.

## Usage

```
lxgraph [OPTIONS]
```

Pipe numbers or `label,value` pairs into `lxgraph`. The chart is rendered
locally in Rust; the LLM is called only to suggest axis labels and chart type.

## Input formats

One number per line:
```
42
17
88
```

Label,value pairs:
```
Sales Q1,1200
Sales Q2,1500
Sales Q3,980
Sales Q4,2100
```

Multi-column CSV (first numeric column is used as the value, preceding string columns become the label):
```
region,product,q1_sales,q2_sales
North,Widget A,12500,15200
South,Widget B,8900,9200
```

Space-separated numbers on a single line:
```
10 20 30 40
```

Lines starting with `#` and blank lines are ignored.

## Output

Plain mode (default) — ASCII chart to stdout, series labels to stderr:
```
Sales Q1 | ████████████░░░░░░░░  1200
Sales Q2 | ███████████████░░░░░  1500
Sales Q3 | █████████░░░░░░░░░░░   980
Sales Q4 | ████████████████████  2100
```

JSON mode (`--json`):
```json
{
  "chart": "Sales Q1 | ████████████░░░░░░░░  1200\n...",
  "series": ["Sales Q1", "Sales Q2", "Sales Q3", "Sales Q4"]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM, then exit |
| `--quiet / -q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language for LLM labels |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Limit bytes read from stdin |
| `--file <PATH>` | Read input from file instead of stdin |
| `--version / -V` | Print version information |
| `--help / -h` | Show help |

## Security flags

None. Chart rendering is entirely local; only the raw input text is sent to
the LLM to suggest labels and chart type.
