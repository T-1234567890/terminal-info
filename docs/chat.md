# Chat Modes

The AI CLI uses one shared chat engine with multiple modes:

- `tinfo chat`
- `tinfo ask`
- `tinfo ai fix`
- `tinfo ai sum`
- `tinfo ai plan`
- `tinfo ai doc`

It is designed for developer workflows, especially:
- quick log analysis from stdin
- file-aware prompts with `@file`
- fast model switching in a terminal session

## Modes

### chat

```bash
tinfo chat
```

- interactive
- multi-turn
- keeps history when enabled

### ask

```bash
tinfo ask "why is this slow"
cat log.txt | tinfo ask
```

- single-shot
- no history
- concise answer
- exits after the response

### fix

```bash
tinfo ai fix @error.log
cat error.log | tinfo ai fix
```

- single-shot
- focused on debugging
- asks the model to explain the problem, cause, and solution

### summarize

```bash
tinfo ai sum @README.md
cat README.md | tinfo ai sum
```

- single-shot
- focused on summarization
- prefers structured, concise output

### plan

```bash
tinfo ai plan "roll out the plugin registry migration"
cat notes.txt | tinfo ai plan
```

- single-shot
- focused on producing a plan
- prompts for Markdown or plain text before the request
- can save the result to a file after output completes

### doc

```bash
tinfo ai doc @src/main.rs
cat notes.txt | tinfo ai doc
```

- single-shot
- focused on writing documentation
- prompts for Markdown or plain text before the request
- can save the result to a file after output completes

## Recommendation

For most users, `OpenRouter` is the best default choice.

Why:
- broader model catalog
- easier model switching
- one provider key for many backends

## Start

```bash
tinfo chat
```

Optional flags:

```bash
tinfo chat --provider openai
tinfo chat --provider claude
tinfo chat --provider openrouter
tinfo chat --model gpt-5.4
tinfo chat --system "You are concise and technical."
tinfo chat --conn my_api
```

One-shot modes support the same flags:

```bash
tinfo ask --provider openrouter "explain this stack trace"
tinfo ai fix --conn my_api @error.log
tinfo ai sum --model gpt-5.4 @notes.txt
tinfo ai plan "migration checklist"
tinfo ai doc @README.md
```

## Plan And Doc Output Format

`plan` and `doc` ask for an output format before the AI request when the terminal is interactive:

```text
Output as Markdown? (Y/n):
```

Behavior:
- default is Markdown
- `Enter` or `y` keeps Markdown output
- `n` switches to plain text

Prompt behavior changes with the selected format:
- Markdown forces headings, sections, and lists
- plain text avoids Markdown formatting

After output completes, `plan` and `doc` can save the result:

```text
Save output to file? (y/N):
```

If yes, `tinfo` asks for a path and defaults to:
- `./output.md` for Markdown
- `./output.txt` for plain text

If the file already exists, it asks before overwriting.

## Killer Workflow: Pipe Into Chat

All AI modes support piped stdin. `chat` treats piped stdin as one-shot analysis mode, and `ask` / `fix` / `sum` / `plan` / `doc` stay single-shot.

Example:

```bash
cat error.log | tinfo chat
cat error.log | tinfo ask
cat error.log | tinfo ai fix
cat README.md | tinfo ai sum
cat outline.txt | tinfo ai plan
cat notes.txt | tinfo ai doc
```

Behavior:
- detects non-interactive stdin
- shows a short detected-input banner, for example:
  - `Input detected (log, 2.3KB)`
  - `Analyzing...`
- wraps the input into a structured analysis prompt
- asks the model to explain it, identify issues, and suggest fixes
- streams one response, then exits

Example output:

```text
Mode: fix
Input detected (log, 2.3KB)
Analyzing...

AI: [streamed response]
```

This is useful for:
- logs
- stack traces
- compiler output
- command output from other tools

## File References With `@file`

All AI modes support `@file` references:

```text
@error.log explain this
@src/main.rs what does this do?
tinfo ai fix @error.log
tinfo ai sum @README.md
tinfo ai plan @plan.txt
tinfo ai doc @README.md
```

Behavior:
- loads the referenced file from disk
- shows a load notice such as `Loaded file: error.log (2.1 KB)`
- injects the file into the prompt as structured context

This keeps the AI CLI terminal-native for debugging and code review workflows.

Notes:
- file loads are size-limited
- missing files fail clearly instead of silently being ignored

## Connections

Connections are config-defined external resources that add context without tool execution.

Commands:

```bash
tinfo connections
tinfo chat --conn my_api
```

When a connection is active, the prompt shows it:

```text
[OpenRouter · model · my_api] >
```

## In-Chat Commands

- `/provider` switch provider
- `/model` switch model
- `/new` start a new chat
- `/chats` open saved chats
- `/clear` clear the screen
- `/copy` copy the last assistant response
- `/exit` leave chat
- `/quit` leave chat

## Provider Models

### OpenAI

- `gpt-5.4`
- `gpt-5.4-mini`
- `gpt-5.4-nano`
- `gpt-5.1`
- `gpt-5-mini`
- `gpt-5-nano`
- `gpt-5-pro`
- `gpt-5`
- `gpt-4.1`
- `o3-deep-research`

### Claude

- `claude-opus-4-6`
- `claude-sonnet-4-6`
- `claude-haiku-4-5`

### OpenRouter

- `z-ai/glm-5v-turbo`
- `stepfun/step-3.5-flash:free`
- `qwen/qwen3.6-plus-preview`
- `nvidia/nemotron-3-super:free`
- `anthropic/claude-4.6-sonnet`
- `anthropic/claude-4.6-opus`
- `openai/gpt-5.4-pro`
- `openai/gpt-5.3-codex`
- `google/gemini-3.1-pro-preview`
- `google/gemini-3.1-flash`
- `deepseek/deepseek-v3.2`
- `deepseek/deepseek-r1`
- `xiaomi/mimo-v2-pro`
- `minimax/minimax-m2.7`
- `x-ai/grok-4.20-multi-agent`
- `x-ai/grok-4.20`
- `meta/llama-4-400b-instruct`
- `mistralai/mistral-large-2603`
- `mistralai/devstral-2-123b`
- `z-ai/glm-5`
- `z-ai/glm-4.5-air`
- `openai/gpt-5.4-nano`
- `openai/gpt-5.4`
- `openai/gpt-oss-120b`
- `moonshotai/kimi-k2.5`
- `liquid/lfm-2.5-thinking`
- `google/gemma-4-31b-dense`

## Custom OpenRouter Models

When using `OpenRouter`, the model picker includes `Custom model...`.

Custom models must use the format:

```text
provider/model
```

Examples:

- `openai/gpt-5.4-pro`
- `anthropic/claude-4.6-sonnet`
- `google/gemini-3.1-flash`

Allowed characters:
- letters and numbers
- `/`
- `.`
- `-`
- `_`
- `:`

Invalid custom values are rejected immediately.

## History And Context

Config is shared through `~/.tinfo/config.toml`.

Relevant settings:

```toml
[ai.runtime]
chat_history = true
chat_context = true
```

- `chat_history = true` saves chats and enables `/chats`
- `chat_context = true` sends prior messages back to the provider
- `chat_context = false` keeps the active chat local, but only sends the latest message

## API Keys

Keys are stored in OS secure storage, not in plaintext config.

`tinfo chat` prompts for a key only after the provider is known.
