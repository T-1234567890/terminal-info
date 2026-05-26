# Math

`tinfo math` is a first-party symbolic math utility for terminal workflows. It is deterministic first: the local symbolic engine parses, simplifies, solves, substitutes, evaluates, derives supported expressions, and builds dependency graphs. AI is only used by `tinfo math explain --ai` to explain deterministic output.

This is not a full computer algebra system. Unsupported operations return explicit diagnostics instead of guessed answers.

## Input

Math commands accept a file or piped stdin:

```bash
tinfo math solve equation.math
cat equation.math | tinfo math solve
echo "a=5\nb=a^2+3" | tinfo math solve
```

Supported input formats:

- `.math`
- `.txt`
- `.json`
- `.tex`
- stdin text

The `.math` and `.txt` formats use one statement per line. Lines starting with `#` are comments.

```text
a = 5
b = a^2 + 3
x + 2 = 5
diff(x^2, x)
assume x > 0
distance = 12 km
time = 30 min
speed = distance / time
```

JSON input can be an array of statement strings or an object:

```json
["a=5", "b=a^2+3"]
```

```json
{
  "a": 5,
  "b": "a^2+3"
}
```

LaTeX input supports simple equation extraction from math blocks. It does not try to parse arbitrary LaTeX documents.

## Solve

```bash
tinfo math solve equation.math
cat equation.math | tinfo math solve
tinfo --json math solve equation.math
tinfo math solve equation.math --steps --trace --vars
tinfo math solve equation.math --latex
tinfo math solve equation.math --precision 4
tinfo math solve equation.math --for x
```

Supported deterministic operations:

- assignments such as `a = 5`
- expressions such as `a^2 + 3`
- simple equations such as `x + 2 = 5`
- substitution from earlier assignments
- constant folding
- identity simplification such as `x + 0`, `x * 1`, and `x * 0`
- dependency extraction
- numeric evaluation when dependencies resolve
- linear single-variable solving
- solving for a specific variable with `--for x`
- quadratic solving for one variable
- small linear systems such as `x + y = 5` and `x - y = 1`
- assumptions such as `assume x > 0` are parsed and reported
- lightweight units for `mm`, `cm`, `m`, `km`, `ms`, `s`, `min`, `h`, `g`, and `kg`
- basic derivatives through `diff(expr, variable)` or `derivative(expr, variable)`
- basic limits through `lim(expr, variable, value)` or `limit(expr, variable, value)`
- basic integrals through `integrate(expr, variable)` or `integral(expr, variable)`

Calculus support includes constants, variables, sums, products, quotients, numeric powers, basic chain rule, and simple functions such as `sin`, `cos`, `tan`, `ln`, `log`, `exp`, and `sqrt` where deterministic rules are implemented.

Unsupported operations are reported as diagnostics.

## Graph

```bash
tinfo math graph equation.math --mermaid
tinfo math graph equation.math --dot
tinfo math graph equation.math --html --output graph.html
tinfo math graph equation.math --html --output graph.html --open
```

The graph command emits variable dependency graphs. An edge such as `b --> a` means `b` depends on `a`.

## Render

```bash
tinfo math render equation.math --md
tinfo math render equation.math --html --output report.html
tinfo math render equation.math --html --theme light --compact
tinfo math render equation.math --pdf
```

Report outputs:

- Markdown with input, results, variables, diagnostics, and traces
- HTML with MathJax, rendered Mermaid dependency graphs, and collapsible trace sections
- PDF is intentionally unsupported in the first version and returns: `PDF export is not supported yet. Generate HTML with --html.`

## Explain

```bash
tinfo math explain equation.math
tinfo math explain equation.math --ai
tinfo math explain equation.math --ai --audience student --style educational
tinfo math explain equation.math --ai --model openrouter
tinfo math explain equation.math --html
tinfo math explain equation.math --html --output explanation.html
tinfo math explain equation.math --ai --output explanation.md
```

Without `--ai`, `explain` renders a deterministic explanation report locally.

With `--ai`, Terminal Info sends only deterministic symbolic output to the configured AI provider. The AI prompt explicitly forbids solving independently or changing results. AI may only explain, summarize, or reword the deterministic result.

Use `--output <path>` to write the explanation or HTML report to a file.

Supported audiences:

- `student`
- `developer`
- `researcher`

Supported styles:

- `concise`
- `educational`
- `formal`

## REPL

```bash
tinfo math repl
```

Example session:

```text
math> a = 5
math> b = a^2 + 3
math> solve
math> graph
math> latex
math> clear
math> exit
```

The REPL keeps statements in memory for the current session only.

## Output Modes

`tinfo math` follows normal Terminal Info output conventions:

- `--plain` for simple terminal output
- `--compact` for one-line summaries
- `--color` for boxed terminal output
- `--json` for machine-readable output where supported

JSON output is available through the global flag:

```bash
tinfo --json math solve equation.math
```

## Current Limits

- The parser supports a small expression grammar, not a full CAS language.
- Assumptions are recorded for reports and future solving behavior; they do not yet drive branch-sensitive simplification.
- Units are lightweight numeric units, not a full dimensional-analysis system.
- Limits use direct substitution and a small deterministic rule set only.
- LaTeX input is simple extraction only.
- PDF export is deferred.
- AI explanations are optional and never determine correctness.
- Unsupported symbolic operations return diagnostics instead of guessed results.
